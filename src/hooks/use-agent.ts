import { streamDeepSeek, DeepSeekError } from "#/agent/providers/deep-seek";
import type { DeepSeekFinishReason } from "#/agent/providers/deep-seek/types";
import type { ProviderOutput } from "#/domain/provider";
import { nanoid } from "nanoid";
import { useCallback, useEffect, useReducer, useRef } from "react";

export type AgentMessageContent =
  | { type: "text"; text: string }
  | { type: "interaction"; name: string; payload: unknown };

export type AgentMessageStatus =
  | "streaming"
  | "completed"
  | "incomplete"
  | "failed"
  | "cancelled";

export interface AgentMessage {
  id: string;
  runId: string;
  // role 决定发送给模型时的语义；source 用来区分相同 role 的真实模型输出与应用层合成消息。
  role: "user" | "assistant" | "system" | "tool";
  source: "user" | "model" | "synthetic" | "system" | "tool";
  content: AgentMessageContent;
  status: AgentMessageStatus;
  error: string | null;
  finishReason: DeepSeekFinishReason | null;
  createdAt: number;
}

export type AgentRunStatus = "running" | "completed" | "failed" | "cancelled";

export interface AgentRun {
  id: string;
  // Run 由至多一条输入消息触发，但可以产生任意数量的输出消息。
  inputMessage: AgentMessage | null;
  outputMessages: AgentMessage[];
  providerId: string | null;
  status: AgentRunStatus;
  error: string | null;
  createdAt: number;
  startedAt: number | null;
  completedAt: number | null;
}

/** 等待阶段只保存启动请求所需的最小数据。 */
export interface PendingRun {
  id: string;
  inputMessage: AgentMessage;
  providerId: string;
  createdAt: number;
}

interface AgentState {
  // 历史区只在 Run 进入终态时追加，流式阶段不会复制或修改这里的大量数据。
  completedRuns: AgentRun[];
  // 当前采用单消费者模型，因此同一时刻最多只有一个 active Run。
  activeRun: AgentRun | null;
  // 这里只包含真正尚未领取的任务，队首永远是下一条待处理任务。
  pendingRuns: PendingRun[];
}

type TerminalRunStatus = "completed" | "failed" | "cancelled";

type Action =
  | { type: "enqueue"; run: PendingRun }
  | { type: "start-next"; startedAt: number }
  | { type: "add-active-output"; message: AgentMessage }
  | { type: "append-active-text"; messageId: string; text: string }
  | {
      type: "finish-active";
      messageId: string;
      status: TerminalRunStatus;
      completedAt: number;
      error?: string;
      finishReason?: DeepSeekFinishReason;
    }
  | { type: "append-completed"; run: AgentRun }
  | { type: "clear" };

const initialState: AgentState = {
  completedRuns: [],
  activeRun: null,
  pendingRuns: [],
};

function reducer(state: AgentState, action: Action): AgentState {
  switch (action.type) {
    case "enqueue":
      return { ...state, pendingRuns: [...state.pendingRuns, action.run] };

    case "start-next": {
      if (state.activeRun) return state;

      const [pendingRun, ...remainingRuns] = state.pendingRuns;
      if (!pendingRun) return state;

      return {
        ...state,
        activeRun: {
          ...pendingRun,
          outputMessages: [],
          status: "running",
          error: null,
          startedAt: action.startedAt,
          completedAt: null,
        },
        // 领取和移除队首在同一次 reducer 更新中完成，不存在游标不同步问题。
        pendingRuns: remainingRuns,
      };
    }

    case "add-active-output":
      if (!state.activeRun || state.activeRun.id !== action.message.runId) {
        return state;
      }

      return {
        ...state,
        activeRun: {
          ...state.activeRun,
          outputMessages: [...state.activeRun.outputMessages, action.message],
        },
      };

    case "append-active-text": {
      if (!state.activeRun) return state;

      // 高频流式更新只复制 activeRun 及其少量 outputMessages，不触碰 completedRuns。
      return {
        ...state,
        activeRun: {
          ...state.activeRun,
          outputMessages: state.activeRun.outputMessages.map((message) => {
            if (
              message.id !== action.messageId ||
              message.content.type !== "text"
            ) {
              return message;
            }

            return {
              ...message,
              content: {
                type: "text",
                text: message.content.text + action.text,
              },
            };
          }),
        },
      };
    }

    case "finish-active": {
      if (!state.activeRun) return state;

      const outputMessages = state.activeRun.outputMessages.map((message) => {
        if (message.id !== action.messageId) return message;

        const hasPartialText =
          message.content.type === "text" && message.content.text.length > 0;

        return {
          ...message,
          status:
            action.status === "failed" && hasPartialText
              ? ("incomplete" as const)
              : action.status,
          error: action.error ?? null,
          finishReason: action.finishReason ?? null,
        };
      });
      const completedRun: AgentRun = {
        ...state.activeRun,
        outputMessages,
        status: action.status,
        error: action.error ?? null,
        completedAt: action.completedAt,
      };
      return {
        // failed / cancelled 也是终态，必须进入历史并释放 active，否则会阻塞后续队列。
        completedRuns: [...state.completedRuns, completedRun],
        activeRun: null,
        pendingRuns: state.pendingRuns,
      };
    }

    case "append-completed":
      return {
        ...state,
        completedRuns: [...state.completedRuns, action.run],
      };

    case "clear":
      return initialState;
  }
}

function getErrorMessage(cause: unknown): string {
  if (cause instanceof DeepSeekError) {
    return `[${cause.code}] ${cause.message}`;
  }

  return "发生未知错误";
}

function getRunMessages(run: AgentRun) {
  return [
    ...(run.inputMessage ? [run.inputMessage] : []),
    ...run.outputMessages,
  ];
}

/**
 * 在 Run 真正开始时构建上下文，而不是在入队时构建。
 * 因此排队期间前序 Run 新产生的输出会被包含，后续 pending 输入不会提前混入。
 * synthetic / interaction / tool 默认只服务 UI，不直接发送给模型。
 */
function buildRunContext(completedRuns: AgentRun[], activeRun: AgentRun) {
  return [
    ...completedRuns.flatMap(getRunMessages),
    ...(activeRun.inputMessage ? [activeRun.inputMessage] : []),
  ]
    .filter(
      (message) =>
        message.status === "completed" &&
        message.content.type === "text" &&
        (message.role === "user" || message.role === "assistant") &&
        (message.source === "user" || message.source === "model"),
    )
    .map((message) => ({
      role: message.role,
      content: message.content.type === "text" ? message.content.text : "",
    }));
}

export function useAgent(provider: ProviderOutput | undefined) {
  const [state, dispatch] = useReducer(reducer, initialState);
  // 命令回调通过 ref 读取最新状态，避免为了读取 activeRun 而频繁重建 callback。
  const stateRef = useRef(state);
  const abortRef = useRef<AbortController | null>(null);
  // React StrictMode 可能重复运行 effect；这个集合保证一个 Run 只启动一个网络请求。
  const executingRunIdsRef = useRef(new Set<string>());
  // 每个 Run 捕获入队时的 Provider，避免等待期间 UI 切换 Provider 改变既有任务配置。
  const providersByRunIdRef = useRef(new Map<string, ProviderOutput>());

  stateRef.current = state;

  const cancelActiveRun = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  const clear = useCallback(() => {
    abortRef.current?.abort();
    providersByRunIdRef.current.clear();
    dispatch({ type: "clear" });
  }, []);

  const appendSyntheticOutput = useCallback((content: AgentMessageContent) => {
    const activeRun = stateRef.current.activeRun;
    const runId = activeRun?.id ?? nanoid();
    const now = Date.now();
    const message: AgentMessage = {
      id: nanoid(),
      runId,
      role: "assistant",
      source: "synthetic",
      content,
      status: "completed",
      error: null,
      finishReason: null,
      createdAt: now,
    };

    if (activeRun) {
      // 处理中产生的 mock / 交互消息属于当前 Run，并与模型输出并列。
      dispatch({ type: "add-active-output", message });
    } else {
      // 没有 active Run 时，合成消息作为一个无 Provider 的独立完成记录追加。
      dispatch({
        type: "append-completed",
        run: {
          id: runId,
          inputMessage: null,
          outputMessages: [message],
          providerId: null,
          status: "completed",
          error: null,
          createdAt: now,
          startedAt: now,
          completedAt: now,
        },
      });
    }

    return message.id;
  }, []);

  const enqueueUserMessage = useCallback(
    (rawContent: string) => {
      const content = rawContent.trim();
      if (!content || !provider) return null;

      const now = Date.now();
      const runId = nanoid();
      const inputMessage: AgentMessage = {
        id: nanoid(),
        runId,
        role: "user",
        source: "user",
        content: { type: "text", text: content },
        status: "completed",
        error: null,
        finishReason: null,
        createdAt: now,
      };
      const run: PendingRun = {
        id: runId,
        inputMessage,
        providerId: provider.id,
        createdAt: now,
      };

      // Provider 密钥不进入 reducer 状态，只保存在运行期映射中。
      providersByRunIdRef.current.set(run.id, provider);
      dispatch({ type: "enqueue", run });
      return run.id;
    },
    [provider],
  );

  // 调度器：active 空闲时领取并移除 pending 队首任务。
  useEffect(() => {
    if (state.activeRun) return;
    if (state.pendingRuns.length > 0) {
      dispatch({ type: "start-next", startedAt: Date.now() });
    }
  }, [state.activeRun, state.pendingRuns]);

  // 执行器：只负责执行 active Run；请求结束后 reducer 会释放 active，调度器再领取下一条。
  useEffect(() => {
    const activeRun = state.activeRun;
    if (!activeRun || executingRunIdsRef.current.has(activeRun.id)) return;

    const runId = activeRun.id;
    const runProvider = providersByRunIdRef.current.get(runId);
    if (!runProvider) return;
    const executionProvider: ProviderOutput = runProvider;

    executingRunIdsRef.current.add(runId);
    const controller = new AbortController();
    abortRef.current = controller;

    const outputMessage: AgentMessage = {
      id: nanoid(),
      runId,
      role: "assistant",
      source: "model",
      content: { type: "text", text: "" },
      status: "streaming",
      error: null,
      finishReason: null,
      createdAt: Date.now(),
    };
    const context = buildRunContext(state.completedRuns, activeRun);

    dispatch({ type: "add-active-output", message: outputMessage });

    async function execute() {
      let chunkBuffer = "";
      let flushTimer: ReturnType<typeof setTimeout> | null = null;

      // SSE chunk 往往很碎。合并后每 40ms 最多触发一次状态更新，降低 reducer 和 Markdown 重渲染频率。
      function flushContent() {
        if (flushTimer) clearTimeout(flushTimer);
        flushTimer = null;
        if (!chunkBuffer) return;

        const text = chunkBuffer;
        chunkBuffer = "";
        dispatch({
          type: "append-active-text",
          messageId: outputMessage.id,
          text,
        });
      }

      function queueContent(text: string) {
        chunkBuffer += text;
        flushTimer ??= setTimeout(flushContent, 40);
      }

      try {
        for await (const event of streamDeepSeek(
          executionProvider.baseUrl,
          executionProvider.key,
          context,
          controller.signal,
        )) {
          if (event.type === "content") {
            queueContent(event.content);
          } else {
            flushContent();
            dispatch({
              type: "finish-active",
              messageId: outputMessage.id,
              status: "completed",
              finishReason: event.reason,
              completedAt: Date.now(),
            });
          }
        }
      } catch (cause) {
        flushContent();
        dispatch({
          type: "finish-active",
          messageId: outputMessage.id,
          status: controller.signal.aborted ? "cancelled" : "failed",
          error: controller.signal.aborted ? undefined : getErrorMessage(cause),
          completedAt: Date.now(),
        });
      } finally {
        // 无论完成、失败还是取消，都清理本次 Run 的运行期资源。
        if (flushTimer) clearTimeout(flushTimer);
        executingRunIdsRef.current.delete(runId);
        providersByRunIdRef.current.delete(runId);
        if (abortRef.current === controller) abortRef.current = null;
      }
    }

    void execute();
  }, [state.activeRun, state.completedRuns]);

  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    [],
  );

  // UI 时间线由三段状态派生：历史 → 当前输入/输出 → 等待中的用户输入。
  const messages = [
    ...state.completedRuns.flatMap(getRunMessages),
    ...(state.activeRun ? getRunMessages(state.activeRun) : []),
    ...state.pendingRuns.map((run) => run.inputMessage),
  ];

  return {
    messages,
    completedRuns: state.completedRuns,
    activeRun: state.activeRun,
    pendingRuns: state.pendingRuns,
    enqueueUserMessage,
    appendSyntheticOutput,
    cancelActiveRun,
    clear,
  };
}
