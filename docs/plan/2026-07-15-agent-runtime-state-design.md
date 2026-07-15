# Agent Runtime 状态设计讨论记录

> 记录日期：2026-07-15
> 当前结论：暂不调整现有实现；先记录领域边界、状态一致性问题和后续演进方向。

---

## 1. 背景

当前 `useAgent` 同时管理一次前端会话中的三类运行数据：

```text
completedRuns：本次会话中已经进入终态的 Run
activeRun：当前正在执行的 Run，最多一个
pendingRuns：等待执行的 Run，FIFO
```

它还承担以下职责：

- 接收用户输入并创建 `PendingRun`。
- 选择并快照本次 Run 使用的 Provider。
- 构建发送给模型的上下文。
- 执行流式 Provider 请求。
- 更新 active output。
- 处理完成、失败和取消。
- active 结束后继续调度 pending 队首。
- 为 UI 派生当前可见消息。

随着多轮对话、任务排队和未来持久化需求出现，需要明确这些数据应该由谁拥有、如何迁移，以及拆分后如何避免 React 异步 state 导致的竞态。

---

## 2. 领域概念

### 2.1 Message 与 Run

`AgentMessage` 表示一条可以展示或发送给模型的消息。

`AgentRun` 表示一次执行和调度单元。它不是固定的“一问一答”，一次 Run 可以产生多条 output，例如：

```text
AgentRun
├── inputMessage
└── outputMessages
    ├── 模型文本
    ├── Tool 输出
    └── 应用层交互消息
```

当前已经确定：一次 Run 由至多一条直接输入消息触发，因此使用：

```ts
interface AgentRun {
  inputMessage: AgentMessage | null;
  outputMessages: AgentMessage[];
}
```

这里的 `inputMessage` 不等于完整模型上下文。多轮历史、补充信息和系统消息应由 Context Builder 组合，而不是复制进当前 Run 的 input。

### 2.2 Message ID 与 Run ID

消息自己的 `id` 是必要的：

- React 列表需要稳定 key。
- 流式更新需要定位具体 output。
- 一个 Run 可以产生多条消息，不能直接复用 Run ID。

当前内存模型把 Message 嵌套在 Run 中，因此 `AgentMessage.runId` 存在冗余嫌疑。父级 Run 已经表达消息归属。

持久化后是否需要 Run 与 Message 的关联，取决于存储方式：

```text
嵌套保存 Run + Messages
→ Message 不需要 runId

拆表保存 Runs + Messages
→ 必须存在关联，但可以使用 message.run_id、run.input_message_id 或关联表
```

因此，领域模型是否携带 `runId` 和数据库是否需要关联不是同一个问题。存储层可以使用独立 DTO 表达外键。

### 2.3 Synthetic Output

`appendSyntheticOutput` 最初用于支持应用层主动创建 assistant 消息，例如确认卡片或交互操作。

当前没有调用方，UI 也没有真正实现 interaction 渲染。它还导致 `inputMessage` 和 `providerId` 必须允许 `null`，并引入额外 reducer action。

当前只记录这个问题，暂不修改。后续应根据真实交互需求决定是否保留，而不是继续提前设计接口。

---

## 3. 三段式当前状态

当前交互数据可以分为三段：

```ts
interface AgentState {
  completedRuns: AgentRun[];
  activeRun: AgentRun | null;
  pendingRuns: PendingRun[];
}
```

### 3.1 Completed

`completedRuns` 保存本次页面/会话周期内已经完成、失败或取消的 Run。

它的主要数据特征是追加：

```ts
completedRuns.push(settledRun);
```

失败和取消同样是终态，必须离开 active，否则会阻塞整个队列。具体结果通过 Run status 区分。

### 3.2 Active

`activeRun` 是高频更新区：

- 创建模型 output。
- 追加流式文本。
- 更新 finish reason。
- 标记完成、失败或取消。

当前使用单消费者模型，同一时间最多执行一个 Run。

流式 chunk 已经在进入 React state 前进行批处理，避免每个 SSE chunk 都触发 reducer 和 Markdown 渲染。

### 3.3 Pending

`pendingRuns` 是 FIFO 队列，只保存启动请求所需的最小数据：

```ts
interface PendingRun {
  id: string;
  inputMessage: AgentMessage;
  providerId: string;
  createdAt: number;
}
```

队列操作应使用 `enqueue/dequeue` 命名，而不是 `push/pop`：

- `enqueue`：加入队尾。
- `dequeue`：取出队首。
- `pop` 在 JavaScript 中通常表示取出队尾，语义是 LIFO。

之前的 `pendingHead` 已被移除。现在 `pendingRuns` 始终等于真实等待列表，领取下一条时同步设置 active 并移除队首。

---

## 4. 持久化历史与本次 Completed 不是同一层

需要区分：

```text
持久化历史
→ 之前已经保存的会话数据

completedRuns
→ 当前交互周期内刚进入终态、供当前 UI 使用的数据
```

未来可以引入只读历史 Hook：

```ts
const historicalRuns = useConversationHistory(conversationId);
```

它只负责读取持久化数据，不收录本次交互缓存，也不直接执行当前 Run 的状态迁移。

当前 UI 数据可以由两部分组合：

```ts
const visibleRuns = [
  ...historicalRuns,
  ...currentCompletedRuns,
  activeRun,
  ...pendingRuns,
];
```

完成数据真正写入数据库时，应由 Repository/Service 负责；历史 Hook 只订阅或重新加载结果。

---

## 5. 为什么不能简单拆成三个独立 State Hook

最直观的拆法是：

```text
useCompletedRuns(useState)
usePendingRuns(useState)
useActiveRun(useState)
```

但 React state 更新不是同步可读取的命令式写入。例如：

```ts
pending.enqueue(run);
const next = pending.dequeue();
```

`enqueue` 只安排一次 React 更新，当前闭包中的 `pendingRuns` 仍然是旧值，紧接着 `dequeue` 可能读不到刚加入的任务。

完成迁移也存在相同问题：

```ts
completed.append(activeRun);
active.clear();
const next = pending.dequeue();
```

这三个操作属于一次业务事务：

```text
active → completed
pending[0] → active
pending.slice(1) → pending
```

如果由三个独立 React state 分别完成，就不存在天然的原子性，中间状态可能互相不一致。

因此必须区分：

```text
逻辑职责拆分 ≠ 物理状态源拆分
```

`completed / active / pending` 是三个职责，但它们当前属于同一个状态机。

---

## 6. 当前安全方案：单一 Reducer

在现阶段，最直接可靠的方案仍然是单一 `useReducer`：

```ts
const [state, dispatch] = useReducer(agentReducer, initialState);
```

所有跨区域迁移由一个 action 完成。

### 6.1 入队

理想的 enqueue reducer 可以基于最新 state 判断：

```text
没有 active
→ 新 Run 直接进入 active

已有 active
→ 新 Run 进入 pending 队尾
```

### 6.2 完成

一次 `finish-active` action 原子完成：

```text
当前 active 追加到 completed
pending 队首转换成新的 active
pending 删除队首
```

网络请求仍然属于副作用，不能放进 reducer。Executor 观察 `activeRun.id`，执行结束后 dispatch 终态 action。

### 6.3 可以拆文件，但共享状态源

即使继续使用单一 reducer，也可以拆分代码职责：

```text
src/agent/runtime/
├── types.ts
├── reducer.ts
├── selectors.ts
├── context-builder.ts
└── executor.ts
```

拆分后仍然只有一份 `AgentState` 和一个统一 dispatch。

---

## 7. 成熟 SDK 如何解决相同问题

### 7.1 Vercel AI SDK

Vercel AI SDK 的 React `useChat` 没有使用多个独立 `useState` 作为真实数据源。

它创建一个长期存在的 `Chat` 实例，消息、status、error 和 active response 由这个实例管理。React 使用 `useSyncExternalStore` 订阅它。

```text
Chat instance
├── state.messages
├── state.status
├── state.error
└── activeResponse

React useChat
└── subscribe(Chat instance)
```

异步方法直接读取 `this.state.messages`，而不是读取某次 React render 捕获的闭包值。流式写入还通过串行 Job Executor 排队，避免并发事件竞态。

它主要支持单个当前请求，没有内建与本项目完全相同的 `pendingRuns[]` 语义。应用如果需要任意任务队列，仍然需要在 Runtime 层增加调度。

### 7.2 LangGraph

LangGraph 把状态、pending tasks 和下一步节点放在统一的 Graph Runtime/Checkpoint 中。

节点只返回状态更新，执行引擎通过 reducer 在 super-step 边界应用更新，因此不会出现多个 React Hook 互相读写旧 state 的问题。

### 7.3 OpenAI Agents SDK

OpenAI Agents SDK 使用 Runner 和 Session：

```text
Runner
├── 读取 Session 历史
├── 执行 Run
├── 产生 output items
└── 写回 Session
```

执行与历史数据由 Runtime/Session 拥有，UI 框架不是状态权威来源。

### 7.4 共同点

成熟 SDK 的共同思路是：

```text
一次会话的数据由一个统一数据源管理
React/Vue/Svelte 只是订阅和命令适配层
```

它们通常选择：

1. 单一 reducer/graph state；或
2. React 外部的 Runtime/Store + subscription。

它们一般不会把一个强关联状态机拆成多个互相命令式调用的 React `useState` Hook。

---

## 8. 未来方向：框架无关 AgentRuntime

如果后续需要真正拆分当前 `useAgent`，推荐把状态机下沉到一个不依赖 React 的 Runtime，而不是创建三个独立 state Hook。

```ts
interface AgentSnapshot {
  completedRuns: AgentRun[];
  activeRun: AgentRun | null;
  pendingRuns: PendingRun[];
}

interface AgentRuntime {
  getSnapshot(): AgentSnapshot;
  subscribe(listener: () => void): () => void;

  enqueueUserMessage(content: string): string | null;
  cancelActiveRun(): void;
  clear(): void;
}
```

Runtime 内部拥有唯一可写状态，并负责：

- 原子状态迁移。
- 队列调度。
- 串行流式写入。
- AbortController 生命周期。
- Provider 执行。
- 通知订阅者。

React Hook 只做适配：

```ts
function useAgent(runtime: AgentRuntime) {
  return useSyncExternalStore(
    runtime.subscribe,
    runtime.getSnapshot,
    runtime.getSnapshot,
  );
}
```

同一个 Runtime 将来可以适配：

```text
React → useSyncExternalStore
Vue → ref/computed
Svelte → readable store
纯 TypeScript → subscribe/getSnapshot
```

Runtime 应按 conversation 创建实例，而不是整个应用共享一个全局单例。

---

## 9. 外部如何读取和写入拆分后的数据

迁移数据所有权不等于移除访问入口。

UI 仍然可以通过聚合 Hook 读取：

```ts
const {
  messages,
  completedRuns,
  activeRun,
  pendingRuns,
  enqueueUserMessage,
  cancelActiveRun,
} = useAgent(runtime);
```

区别在于：

- UI 读取 Runtime snapshot。
- UI 通过领域命令请求修改。
- UI 不直接调用 `setPendingRuns` 或修改数组。

遵循以下约束：

```text
谁拥有数据，谁负责修改数据。
外部只能读取 snapshot 或调用明确命令。
```

例如队列内部可以拥有 `enqueue/dequeue`，但 UI 只暴露 `enqueueUserMessage`、`cancelPendingRun` 等业务命令。`dequeue` 属于调度器内部能力，不应该直接暴露给页面。

---

## 10. 当前决策

本次讨论只形成记录，暂不修改代码。

当前决策如下：

1. 当前交互的 `completed / active / pending` 继续保留在一个统一状态源中。
2. 暂不拆成三个各自拥有 React state 的 Hook。
3. 持久化历史未来独立为只读查询层和 Repository。
4. 如果需要进一步解耦，优先迁移到框架无关的 `AgentRuntime`。
5. React `useAgent` 最终应成为轻量订阅和命令适配层。
6. Runtime 内部仍需显式处理原子迁移、串行写入和请求取消；外部 Store 不会自动消除竞态。
7. `appendSyntheticOutput` 和 `AgentMessage.runId` 的清理另行决策，本次不调整。

---

## 11. 参考资料

- [Vercel AI SDK `useChat` 源码](https://github.com/vercel/ai/blob/main/packages/react/src/use-chat.ts)
- [Vercel AI SDK Chat Runtime 源码](https://github.com/vercel/ai/blob/main/packages/ai/src/ui/chat.ts)
- [Vercel AI SDK `useChat` API](https://ai-sdk.dev/docs/reference/ai-sdk-ui/use-chat)
- [Vercel AI SDK 消息持久化](https://ai-sdk.dev/docs/ai-sdk-ui/chatbot-message-persistence)
- [LangGraph Persistence](https://docs.langchain.com/oss/javascript/langgraph/persistence)
- [OpenAI Agents SDK Sessions](https://openai.github.io/openai-agents-js/guides/sessions/)
