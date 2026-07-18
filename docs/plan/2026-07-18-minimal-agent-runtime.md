# 最简 Agent Runtime 改造方案

> 日期：2026-07-18
>
> 状态：待实施
>
> 基线：`src/hooks/use-agent.ts`、`src/agent/providers/deep-seek/*`
>
> 目标：在不推翻现有 Chat、Run 队列和流式渲染的前提下，把当前“一次 Run 只发起一次模型请求”改造成最简 `model → tool → model` Agent loop，并为后续 MCP 接入提供稳定边界。

---

## 1. 本次决策

本次不直接把整个 `useAgent` 重写成大型 SDK，也不先实现完整 MCP 协议栈。采用下面的最小演进路线：

```text
React useAgent
├── 继续拥有 UI 消息、Run 队列和 active/pending/completed 状态
└── 把 active Run 交给框架无关的 Agent Runner
      ├── 请求模型
      ├── 收集文本和 Tool Call
      ├── 通过 Tool Registry 执行工具
      ├── 把 Tool Result 加回模型上下文
      └── 循环直到最终文本、取消、失败或达到步数上限
```

Tool Registry 是 Runtime 唯一认识的工具边界：

```text
Agent Runner
    ↓
Tool Registry
    ├── 本地 TypeScript Tool
    ├── Tauri Tool Adapter
    └── MCP Tool Adapter（后续）
```

因此 MCP 是一种 Tool 来源和执行适配器，不进入 Agent loop 的核心判断代码。

---

## 2. 当前实现基线

当前 `useAgent` 已经具备以下可以保留的能力：

- `completedRuns / activeRun / pendingRuns` 三段状态。
- 同一时间只执行一个 active Run，pending Run 使用 FIFO。
- Provider 在 Run 入队时快照，密钥不进入 reducer state。
- `AbortController` 取消当前请求。
- StrictMode 下用 `executingRunIdsRef` 防止重复执行。
- SSE 文本增量先缓冲，最多每 40ms 更新一次 React state。
- Run 在真正开始时构建上下文，能包含排队期间刚完成的前序 Run。
- 失败和取消都会释放 active Run，不阻塞后续队列。

当前限制集中在执行器：

```text
active Run
→ streamDeepSeek(...)
→ content delta
→ finish
→ 整个 Run 结束
```

具体问题如下：

1. `useAgent` 直接依赖 `streamDeepSeek`，Runtime 和 Provider 协议耦合。
2. `DeepSeekStreamEvent` 只有 `content / finish`，无法表达 Tool Call。
3. `DeepSeekFinishReason` 不包含工具调用结束语义。
4. `buildRunContext` 会过滤掉 `tool / interaction / synthetic`，无法构造下一步模型请求。
5. `finish-active` 同时结束当前文本消息和整个 Run，不能表达“本轮模型输出结束，但 Run 还要执行工具并进入下一步”。
6. active Run 启动时预先创建唯一 assistant 文本消息；Agent Run 实际可能产生多条文本、Tool Call 和 Tool Result。
7. Provider 只接受 `{ role, content: string }[]`，不能表达结构化工具消息。

本次改造应保留已有状态机优点，只替换上面的单次执行假设。

---

## 3. MVP 目标与非目标

### 3.1 MVP 必须支持

一次用户消息触发一个 `AgentRun`，一个 Run 内允许发生：

```text
Step 1：模型流式输出 + Tool Call
Tool：执行并产生 Tool Result
Step 2：模型读取 Tool Result 后流式输出最终答案
Run：完成
```

必须具备：

- 纯文本请求保持现有流式体验。
- 一个 Run 内可以发起多次模型请求。
- 模型可以返回一个或多个 Tool Call。
- 第一版多个 Tool Call 按顺序执行。
- Tool 失败转换为 `isError: true` 的 Tool Result，再交还模型。
- Provider 失败终止当前 Run。
- `maxSteps` 防止无限工具循环，默认值为 `5`。
- 取消信号贯穿模型流、Tool 执行和下一轮循环。
- UI 能显示文本、Tool Call、Tool Result、失败和取消。
- pending Run 的调度语义保持不变。

### 3.2 本次不实现

- Skill 发现和加载。
- 多 Agent、子 Agent、Agent handoff。
- 并行 Tool 执行。
- 长期任务恢复和 checkpoint。
- 自动上下文压缩。
- 权限确认 UI。
- Tool Search 和动态按需加载。
- 完整 MCP Server 管理页面。
- 在浏览器前端直接启动 stdio MCP Server。
- 将整个状态源立即迁移到 `useSyncExternalStore`。

最后一项是刻意的渐进决策：本次先把执行循环移出 React，确认事件和消息模型稳定后，再把队列与 snapshot 所有权迁入长期存在的 `AgentRuntime` 实例。

---

## 4. 一次 Run 的目标时序

用户输入“查看明天的日历”时，前端看到一个 Run，内部可能有两个模型请求和一个工具请求：

```text
ChatScreen
  │ enqueueUserMessage
  ▼
useAgent reducer
  │ activeRun
  ▼
runAgent()
  │ Step 1: messages + tool specs
  ▼
LlmProvider.stream()
  │ text delta / tool call / finish
  ▼
ToolRegistry.execute()
  │ local tool 或 MCP adapter
  ▼
Tool Result
  │ 追加到本次 transcript
  ▼
LlmProvider.stream()
  │ Step 2: final text
  ▼
run_completed
  │
  ▼
useAgent reducer → completedRuns
```

需要明确三个不同 ID：

- `runId`：一次用户输入触发的完整 Agent Run。
- `step`：Run 内第几次模型调用，从 `1` 开始。
- `toolCallId`：模型生成的某次工具调用，用来关联 Tool Result。

---

## 5. 领域类型

领域类型不能继续由 DeepSeek 响应结构定义。新增 Provider 无关的 Agent 类型。

### 5.1 模型上下文消息

```ts
export type ModelContentBlock =
  | { type: "text"; text: string }
  | {
      type: "tool_call";
      id: string;
      name: string;
      input: Record<string, unknown>;
    }
  | {
      type: "tool_result";
      toolCallId: string;
      output: unknown;
      isError: boolean;
    };

export interface ModelMessage {
  role: "user" | "assistant" | "tool" | "system";
  content: ModelContentBlock[];
}
```

这是 Agent Runner 的 canonical transcript。Provider Adapter 负责把它转换成各家 API 格式。

### 5.2 工具定义和结果

```ts
export interface ToolSpec {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ToolResult {
  toolCallId: string;
  toolName: string;
  output: unknown;
  isError: boolean;
}
```

### 5.3 Provider 事件

Provider 负责解析原始 SSE，并只向 Runner 输出统一事件：

```ts
export type LlmEvent =
  | { type: "text_delta"; delta: string }
  | { type: "tool_call"; call: ToolCall }
  | {
      type: "finish";
      reason: "stop" | "tool_calls" | "length" | "content_filter";
    };

export interface LlmProvider {
  stream(input: {
    messages: ModelMessage[];
    tools: ToolSpec[];
    signal: AbortSignal;
  }): AsyncIterable<LlmEvent>;
}
```

流式 Tool Call 参数可能由多个 SSE delta 拼成。拼接和 JSON 解析属于 Provider Adapter 的职责；Runner 只接收完整、已校验为对象的 `tool_call` 事件。

### 5.4 Runtime 对 UI 的事件

```ts
export type AgentEvent =
  | { type: "run_started"; runId: string }
  | { type: "step_started"; runId: string; step: number }
  | { type: "text_started"; runId: string; step: number; messageId: string }
  | { type: "text_delta"; messageId: string; delta: string }
  | { type: "text_completed"; messageId: string }
  | { type: "tool_call"; runId: string; step: number; call: ToolCall }
  | { type: "tool_result"; runId: string; step: number; result: ToolResult }
  | { type: "run_completed"; runId: string; steps: number }
  | { type: "run_failed"; runId: string; error: string }
  | { type: "run_cancelled"; runId: string }
  | { type: "max_steps_reached"; runId: string; maxSteps: number };
```

UI 事件与 Provider 事件必须分开。Provider 的 `finish` 表示一次模型调用结束；`run_completed` 才表示整个 Agent Run 结束。

---

## 6. Tool Registry 与 MCP 边界

Runner 不直接写任何 `if (toolName === ...)`，也不直接调用 MCP Client。

```ts
export interface AgentTool {
  spec: ToolSpec;
  execute(
    input: Record<string, unknown>,
    context: {
      runId: string;
      step: number;
      signal: AbortSignal;
    },
  ): Promise<unknown>;
}

export interface ToolExecutionContext {
  runId: string;
  step: number;
  signal: AbortSignal;
}

export interface ToolRegistry {
  list(): ToolSpec[];
  execute(call: ToolCall, context: ToolExecutionContext): Promise<ToolResult>;
}
```

Registry 负责：

- 按唯一名称注册工具。
- 防止重复名称覆盖。
- 输出发送给模型的 Tool Specs。
- 使用 JSON Schema 校验模型参数；MCP schema 不能直接用 Zod 解析，实施时增加 `ajv` 作为统一 validator。
- 将未知工具转换为失败 Tool Result。
- 捕获工具异常并脱敏后转换为失败 Tool Result。
- 把 `AbortSignal` 传给 Tool。

### 6.1 MCP Adapter

未来接入 MCP 时，用 Adapter 把 `tools/list` 结果注册为普通 `AgentTool`：

```ts
interface McpClient {
  listTools(): Promise<McpTool[]>;
  callTool(input: {
    name: string;
    arguments: Record<string, unknown>;
    signal: AbortSignal;
  }): Promise<McpToolResult>;
}
```

注册时增加 server namespace，避免多个 MCP Server 工具重名：

```text
calendar__list_events
github__list_issues
```

执行映射：

```text
模型 Tool Call：calendar__list_events
→ Registry 找到 McpToolAdapter
→ Adapter 映射回原始 MCP Tool：list_events
→ McpClient tools/call
→ MCP Tool Result
→ Runtime ToolResult
```

本项目是 Tauri 桌面应用。真实 stdio MCP Client 后续优先放到 Rust/Tauri 侧，前端通过 command/channel 调用；这样进程管理、凭据和本地权限不进入浏览器上下文。Runtime 的 `ToolRegistry` 接口保持不变，因此迁移执行位置不影响 Agent loop。

---

## 7. 最简 Agent Runner

Runner 是纯 TypeScript 异步生成器，不依赖 React：

```ts
export async function* runAgent(input: {
  runId: string;
  messages: ModelMessage[];
  provider: LlmProvider;
  tools: ToolRegistry;
  signal: AbortSignal;
  maxSteps?: number;
}): AsyncGenerator<AgentEvent> {
  const transcript = [...input.messages];
  const maxSteps = input.maxSteps ?? 5;

  yield { type: "run_started", runId: input.runId };

  for (let step = 1; step <= maxSteps; step++) {
    throwIfAborted(input.signal);
    yield { type: "step_started", runId: input.runId, step };

    // collectModelTurn 本身也是 AsyncGenerator：实时 yield 文本事件，
    // stream 结束时 return 完整 assistant message 和 Tool Calls。
    const turn = yield* collectModelTurn({
      runId: input.runId,
      step,
      provider: input.provider,
      messages: transcript,
      tools: input.tools.list(),
      signal: input.signal,
    });

    transcript.push(turn.assistantMessage);

    if (turn.toolCalls.length === 0) {
      yield { type: "run_completed", runId: input.runId, steps: step };
      return;
    }

    for (const call of turn.toolCalls) {
      throwIfAborted(input.signal);
      yield { type: "tool_call", runId: input.runId, step, call };

      const result = await input.tools.execute(call, {
        runId: input.runId,
        step,
        signal: input.signal,
      });

      transcript.push(toToolResultMessage(result));
      yield { type: "tool_result", runId: input.runId, step, result };
    }
  }

  yield {
    type: "max_steps_reached",
    runId: input.runId,
    maxSteps,
  };
}
```

实现时需保证：

- assistant 的文本和 Tool Call 必须先完整加入 transcript。
- Tool Result 必须使用原始 `toolCallId` 回填。
- Tool 返回错误不抛出终止 Run，而是继续下一次模型调用。
- Provider/协议错误才进入 `run_failed`。
- 如果模型产生 Tool Call，即使同一步也产生了文本，Run 仍继续执行工具。
- 达到 `maxSteps` 后不能再发模型请求。

---

## 8. `useAgent` 的具体改造

### 8.1 保留内容

第一阶段继续保留：

- `AgentState` 三段结构。
- pending FIFO 调度 effect。
- Provider 入队快照。
- `executingRunIdsRef`。
- `AbortController` 生命周期。
- `messages` selector。
- 40ms 文本批处理策略。

### 8.2 删除的直接依赖

`useAgent.ts` 不再：

```ts
import { streamDeepSeek } from "#/agent/providers/deep-seek";
```

改为通过 Provider Factory 创建统一 `LlmProvider`，再调用 `runAgent`：

```ts
const llmProvider = createLlmProvider(executionProvider);

for await (const event of runAgent({
  runId,
  messages: context,
  provider: llmProvider,
  tools: toolRegistry,
  signal: controller.signal,
})) {
  handleAgentEvent(event);
}
```

### 8.3 Reducer Action 拆分

当前 `finish-active` 同时完成文本消息和 Run，需要拆成：

```ts
type Action =
  | { type: "enqueue"; run: PendingRun }
  | { type: "start-next"; startedAt: number }
  | { type: "add-active-output"; message: AgentMessage }
  | { type: "append-active-text"; messageId: string; text: string }
  | { type: "complete-active-output"; messageId: string }
  | {
      type: "settle-active";
      status: "completed" | "failed" | "cancelled";
      completedAt: number;
      error?: string;
    }
  | { type: "clear" };
```

语义变化：

```text
complete-active-output
→ 只完成一条文本或工具消息，active Run 仍然运行

settle-active
→ 才将整个 active Run 移入 completedRuns 并释放队列
```

### 8.4 UI Message 扩展

在保持“一条 UI Message 对应一个可渲染单元”的前提下，扩展现有 union：

```ts
export type AgentMessageContent =
  | { type: "text"; text: string }
  | { type: "tool_call"; call: ToolCall }
  | { type: "tool_result"; result: ToolResult }
  | { type: "interaction"; name: string; payload: unknown };
```

每次 `text_started` 创建一条空的 assistant text message；后续 `text_delta` 使用已有批处理追加。Tool Call 和 Tool Result 各创建一条已完成消息。

不要再在 Run 开始时预创建唯一 assistant output，因为某一步可能只有 Tool Call，没有文本。

### 8.5 Context Builder

当前 `buildRunContext` 只发送完成的 user/model 文本。改造后分两层：

```text
buildConversationTranscript(completedRuns, activeRun)
→ 生成 Provider 无关的 ModelMessage[]

Provider Adapter
→ 将 ModelMessage[] 转成 DeepSeek 兼容 API 格式
```

上下文规则：

- completed user text：进入上下文。
- completed model text：进入上下文。
- completed model Tool Call：进入上下文。
- completed Tool Result：进入上下文。
- synthetic interaction：默认不进入上下文。
- failed/cancelled/incomplete 文本：默认不进入下一次 Run。
- 当前 active Run 的 Tool Call/Result 由 Runner 自己维护，不依赖 React state 异步回读。

最后一条很重要：Runner 必须使用自己的局部 transcript 完成当前 Run，不能 dispatch 后立刻从 React state 读取 Tool Result。

---

## 9. Provider 改造

现有 DeepSeek Provider 按下面顺序演进：

1. 将 `streamDeepSeek` 封装为 `LlmProvider.stream`。
2. 请求参数增加 `tools`。
3. Parser 支持文本 delta 和 Tool Call delta。
4. Tool Call 参数在 Provider 内按 `index/id` 累积。
5. Tool Call 结束时解析完整 JSON，并产生统一 `tool_call` 事件。
6. 将 Provider 的 finish reason 映射为内部 `stop / tool_calls / length / content_filter`。
7. Provider 错误继续使用结构化错误，但 `useAgent` 不再识别 DeepSeek 专属错误类型。

建议新增统一错误：

```ts
export class AgentError extends Error {
  constructor(
    readonly code:
      | "PROVIDER_ERROR"
      | "INVALID_PROVIDER_EVENT"
      | "TOOL_ERROR"
      | "MAX_STEPS",
    message: string,
    readonly cause?: unknown,
  ) {
    super(message);
  }
}
```

`AgentMessage.finishReason` 同时从 `DeepSeekFinishReason` 迁移为 Provider 无关的 `AgentFinishReason`；`getErrorMessage` 也只处理统一 `AgentError`，避免 `useAgent` 残留 Provider 类型。

API Key、Authorization header 和完整敏感请求体不得进入事件、消息或错误日志。

---

## 10. 文件规划

```text
src/agent/
├── runtime/
│   ├── types.ts              # ModelMessage、ToolCall、ToolResult、AgentEvent
│   ├── runner.ts             # model → tool → model loop
│   ├── collect-model-turn.ts # 消费一次 Provider stream，形成完整 assistant turn
│   ├── context.ts            # Run 历史 → ModelMessage[]
│   └── errors.ts
├── providers/
│   ├── types.ts              # LlmProvider、LlmEvent
│   ├── factory.ts            # ProviderOutput → LlmProvider
│   └── deep-seek/
│       ├── index.ts
│       ├── parser.ts
│       ├── types.ts
│       └── error.ts
├── tools/
│   ├── types.ts              # AgentTool、ToolRegistry 接口
│   ├── registry.ts
│   └── current-time.ts       # MVP 验证工具
└── mcp/
    ├── types.ts              # 暂时只定义 McpClient 边界
    └── tool-adapter.ts       # MCP Tool ↔ AgentTool，真实 transport 后续实现

src/hooks/
└── use-agent.ts              # 队列、React reducer、Runtime 事件适配
```

`current-time.ts` 只用于证明 loop 正确，不代表 Runtime 依赖本地工具。真实 MCP 接入时删除或保留为开发 fixture 均可。

---

## 11. 实施步骤

### Step 1：抽取 Provider 无关类型

- 新建 `runtime/types.ts` 和 `providers/types.ts`。
- 把 DeepSeek 的文本事件映射为 `LlmEvent`。
- 暂时不增加工具，确保纯文本 Chat 行为不变。

验收：`pnpm build` 通过；现有多轮文本、排队、停止和 40ms 批处理不退化。

### Step 2：实现无工具 Runner

- 新建 `collect-model-turn.ts` 和 `runner.ts`。
- `useAgent` 改为调用 Runner。
- 拆分 `complete-active-output` 与 `settle-active`。

验收：一次纯文本 Run 内只有一个 step，结束后 pending 队首正常启动。

### Step 3：加入 Tool Call 协议

- Provider 请求携带 Tool Specs。
- Parser 支持 Tool Call 参数流式累积。
- Runner 保存 assistant Tool Call 和 Tool Result。
- 增加 `maxSteps = 5`。

验收：Fake Provider 可以稳定执行“第一步 Tool Call、第二步最终文本”。

### Step 4：实现 Tool Registry 和演示工具

- 注册只读 `get_current_time`。
- Tool 输入使用 JSON Schema 校验。
- 未知工具和执行异常返回失败 Tool Result。
- 第一版多个 Tool Call 串行执行。

验收：用户询问当前时间时，同一个 Run 至少产生两次模型调用，并显示 Tool Call、Tool Result 和最终文本。

### Step 5：适配 UI 消息

- Chat 根据 `content.type` 渲染文本、Tool Call 和 Tool Result。
- Tool 消息先使用简单可折叠卡片，不做复杂交互。
- 保持 `useChatScroll(messages)` 接口不变。

验收：用户能从 UI 看出一次 Run 中发生了哪些 Tool 调用，但看不到敏感 header 和 Key。

### Step 6：定义并验证 MCP Adapter

- 定义 `McpClient` 接口。
- 使用 fake MCP Client 测试 `tools/list → registry → tools/call` 映射。
- 确认 namespace、错误和取消语义。
- 再决定真实 MCP Client 放入 Tauri Rust 的具体实现。

验收：切换本地 Tool 与 fake MCP Tool 时，Runner 和 `useAgent` 不需要修改。

---

## 12. 测试计划

建议在开始 Runner 改造时加入 Vitest；核心循环不能只依赖真实模型手测。

### 12.1 Runner 单元测试

使用 Fake Provider 和 Fake Tool Registry：

1. 一步纯文本后完成。
2. 第一步 Tool Call，第二步最终文本。
3. 同一步多个 Tool Call 按顺序执行。
4. Tool 抛错后生成失败 Tool Result，模型下一步仍能继续。
5. 未知 Tool 返回失败 Tool Result。
6. Provider 在第二步失败，Run 失败。
7. 连续调用工具直到达到 `maxSteps`。
8. 模型流中取消后不执行 Tool。
9. Tool 执行中取消后不再发下一次模型请求。

### 12.2 Reducer 测试

1. 一条 active Run 可以追加多条 output。
2. 完成某条 output 不会释放 active Run。
3. `settle-active` 原子完成 active → completed。
4. 失败和取消后 pending 队首仍能继续。
5. 旧 Run 事件不能修改新 active Run。

### 12.3 Provider Parser 测试

1. 文本 SSE delta。
2. Tool Call arguments 被拆成多个 chunk。
3. 同一步多个 Tool Call 交错出现。
4. 无效 Tool JSON。
5. 流结束但没有 finish reason。
6. Provider error payload。

### 12.4 手动回归

- 连续发送两条消息，第二条进入 pending。
- 第一条纯文本正常流式显示。
- Tool Run 中点击停止，Run 进入 cancelled。
- 停止后 pending Run 可以继续。
- 切换 Provider 不影响已入队 Run 的 Provider 快照。
- 清空对话会中止 active Run 并清理运行期 Provider。

---

## 13. 可观察性

开发阶段每个 Run 至少记录以下脱敏字段：

```ts
interface AgentTraceEvent {
  runId: string;
  step: number;
  type:
    | "model_started"
    | "model_completed"
    | "tool_started"
    | "tool_completed"
    | "run_completed"
    | "run_failed";
  providerId?: string;
  toolName?: string;
  durationMs?: number;
  isError?: boolean;
}
```

这可以解释“用户只发送一次消息，后台为什么出现多次请求”：所有模型调用和 Tool 调用共享同一个 `runId`，通过 `step` 和 `toolCallId` 区分。

禁止记录：

- Provider API Key。
- Authorization header。
- 未脱敏的完整 Tool Result。
- 可能包含凭据的完整 MCP 参数。

---

## 14. 完成定义

- [ ] `useAgent` 不再直接 import `streamDeepSeek`。
- [ ] Provider 已转换为统一 `LlmProvider` 接口。
- [ ] 一个 Run 可以包含多个模型 step。
- [ ] Runtime 可以执行 Tool Call，并把 Tool Result 交回模型。
- [ ] Tool Registry 可以同时承载本地 Tool 和 MCP Adapter Tool。
- [ ] Tool Call、Tool Result 和最终文本都出现在同一个 Run 的时间线中。
- [ ] `complete-active-output` 与 `settle-active` 已分离。
- [ ] 取消信号覆盖模型请求、Tool 和 Agent loop。
- [ ] `maxSteps` 可以终止无限循环。
- [ ] pending FIFO 和 Provider 快照行为不退化。
- [ ] Fake Provider 场景测试覆盖核心 Agent loop。
- [ ] `pnpm build` 和 `pnpm check` 通过。

---

## 15. 与现有文档的关系

- 本文落实 `2026-07-15-agent-runtime-state-design.md` 中“将执行器下沉为框架无关 Runtime”的方向，但第一阶段暂不迁移整个 React 状态源。
- 本文收敛 `2026-07-10-frontend-agent-learning.md` 中 Provider、Tool Registry 和 Agent loop 的设计，并以当前已经存在的 `useAgent` 为真实迁移基线。
- 旧文档中“暂不实现 MCP”仍适用于核心 loop：先用本地/fake Tool 验证循环；本文新增 MCP Adapter 边界，为下一阶段接入真实 MCP 做准备。

下一步实施应从 **Step 1：抽取 Provider 无关类型** 开始，不同时改 UI、MCP transport 和 Provider Tool Call parser。
