import { streamDeepSeek, DeepSeekError } from "#/agent/providers/deep-seek";
import type { DeepSeekFinishReason } from "#/agent/providers/deep-seek/types";
import type { ProviderOutput } from "#/domain/provider";
import { nanoid } from "nanoid";
import { useCallback, useEffect, useReducer, useRef } from "react";

export type AssistantMessageStatus =
  | "streaming"
  | "complete"
  | "incomplete"
  | "failed"
  | "cancelled";

export type AgentMessage =
  | {
      id: string;
      role: "user";
      content: string;
    }
  | {
      id: string;
      role: "assistant";
      content: string;
      status: AssistantMessageStatus;
      error: string | null;
      finishReason: DeepSeekFinishReason | null;
    };

type Action =
  | { type: "start"; user: AgentMessage; assistant: AgentMessage }
  | { type: "chunk"; id: string; content: string }
  | { type: "finish"; id: string; reason: DeepSeekFinishReason }
  | { type: "fail"; id: string; error: string }
  | { type: "cancel"; id: string }
  | { type: "clear" };

function reducer(messages: AgentMessage[], action: Action): AgentMessage[] {
  if (action.type === "start") {
    return [...messages, action.user, action.assistant];
  }

  if (action.type === "clear") return [];

  return messages.map((message) => {
    if (message.id !== action.id || message.role !== "assistant") {
      return message;
    }

    switch (action.type) {
      case "chunk":
        return { ...message, content: message.content + action.content };
      case "finish":
        return {
          ...message,
          status: "complete",
          finishReason: action.reason,
        };
      case "fail":
        return {
          ...message,
          status: message.content ? "incomplete" : "failed",
          error: action.error,
        };
      case "cancel":
        return { ...message, status: "cancelled" };
      default:
        return message;
    }
  });
}

function getErrorMessage(cause: unknown): string {
  if (cause instanceof DeepSeekError) {
    return `[${cause.code}] ${cause.message}`;
  }

  return "发生未知错误";
}

export function useAgent(provider: ProviderOutput | undefined) {
  const [messages, dispatch] = useReducer(reducer, []);
  const messagesRef = useRef(messages);
  const abortRef = useRef<AbortController | null>(null);
  const activeAssistantIdRef = useRef<string | null>(null);

  messagesRef.current = messages;

  const stop = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  const clear = useCallback(() => {
    abortRef.current?.abort();
    dispatch({ type: "clear" });
  }, []);

  const sendMessage = useCallback(
    async (rawContent: string) => {
      const content = rawContent.trim();
      if (!content || !provider || abortRef.current) return false;

      const userMessage: AgentMessage = {
        id: nanoid(),
        role: "user",
        content,
      };
      const assistantMessage: AgentMessage = {
        id: nanoid(),
        role: "assistant",
        content: "",
        status: "streaming",
        error: null,
        finishReason: null,
      };
      const controller = new AbortController();

      abortRef.current = controller;
      activeAssistantIdRef.current = assistantMessage.id;
      dispatch({
        type: "start",
        user: userMessage,
        assistant: assistantMessage,
      });

      const history = [...messagesRef.current, userMessage]
        .filter((message) => message.content.length > 0)
        .map(({ role, content: messageContent }) => ({
          role,
          content: messageContent,
        }));

      try {
        for await (const event of streamDeepSeek(
          provider.baseUrl,
          provider.key,
          history,
          controller.signal,
        )) {
          if (event.type === "content") {
            dispatch({
              type: "chunk",
              id: assistantMessage.id,
              content: event.content,
            });
          } else {
            dispatch({
              type: "finish",
              id: assistantMessage.id,
              reason: event.reason,
            });
          }
        }
      } catch (cause) {
        if (controller.signal.aborted) {
          dispatch({ type: "cancel", id: assistantMessage.id });
        } else {
          dispatch({
            type: "fail",
            id: assistantMessage.id,
            error: getErrorMessage(cause),
          });
        }
      } finally {
        if (abortRef.current === controller) abortRef.current = null;
        if (activeAssistantIdRef.current === assistantMessage.id) {
          activeAssistantIdRef.current = null;
        }
      }

      return true;
    },
    [provider],
  );

  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    [],
  );

  return {
    messages,
    isRunning: messages.some(
      (message) =>
        message.role === "assistant" && message.status === "streaming",
    ),
    sendMessage,
    stop,
    clear,
  };
}
