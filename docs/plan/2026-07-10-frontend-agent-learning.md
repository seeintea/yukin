# 前端 Agent 学习与实现计划

> 创建日期：2026-07-10  
> 目标：先使用熟悉的 TypeScript 和 React，在前端完整实现一套可运行、可观察的 Agent，再把已经理解的核心逻辑逐步迁移到 Rust。  
> 范围：本计划只维护一个文档，不再拆分为多个 Phase 文件。

---

## 1. 背景

当前项目原本采用“薄前端、重 Rust”的架构：LLM 请求、流式解析、Agent loop、工具调度、取消控制和密钥读取都计划在 Rust 中完成，React 只负责输入和渲染。

这种架构适合作为最终产品架构，但它把多个陌生概念叠加在了一起：

- Rust 语言、所有权和异步编程
- Tauri command 与 Channel
- Anthropic Messages API 和 SSE
- Agent loop
- Tool use 协议
- Session、Memory 与上下文管理

为了先理解 Agent 本身，本阶段将核心逻辑暂时放到 TypeScript 前端实现。实现过程中保留与 Rust 模块的一一对应关系，确保后续迁移时只是更换执行位置，而不是重新设计整个系统。

---

## 2. 学习目标

完成本计划后，应当能够清楚解释并独立实现以下流程：

1. 消息如何转换为模型 API 请求。
2. 流式响应如何被读取并转换为应用内部事件。
3. 模型如何声明工具调用，以及工具参数如何产生。
4. Agent 为什么需要循环，而不是只请求一次模型。
5. 工具执行结果如何重新加入对话上下文。
6. 一次 Agent run 如何结束、失败或被取消。
7. Session、Message、Memory 分别解决什么问题。
8. 哪些能力可以安全地留在前端，哪些能力最终必须迁回 Rust。

本阶段的重点不是快速堆叠功能，而是让每一层都可观察、可调试、可解释。

---

## 3. 架构演进

### 学习阶段：前端 Agent

```text
React Chat UI
      │
      ▼
TypeScript Agent Runner
      ├── Anthropic Provider（fetch + SSE）
      ├── Tool Registry
      ├── Browser-safe Tools
      ├── AbortController
      └── AgentEvent
      │
      ├── 直接请求 Anthropic API（仅限学习/开发）
      │
      └── invoke Tauri Commands
            ├── Session / Memory
            └── Workspace File API
```

### 最终阶段：迁回 Rust

```text
React Chat UI
      │ invoke + Channel<AgentEvent>
      ▼
Rust Agent Runner
      ├── Anthropic Provider（reqwest + SSE）
      ├── Tool Registry
      ├── Local Tools
      ├── CancellationToken
      └── SQLite / Keychain / Workspace
```

前后两套实现使用相同的领域概念和事件模型，以便逐层替换。

---

## 4. 安全边界

前端直接请求模型 API 只用于本地学习和开发，不作为最终发布方案。

需要明确以下约束：

- API Key 会进入 Tauri WebView 的 JavaScript 运行环境。
- 不将 API Key 写入源码、Git、日志、错误信息或消息记录。
- 不使用 Vite 的 `VITE_*` 环境变量保存真实密钥，因为它会被打进前端产物。
- 学习阶段可以在设置页临时输入 Key，并只保存在当前页面内存中；刷新后失效。
- 前端不得直接执行任意 Shell 命令。
- 本地文件仍通过现有 Tauri command 访问，并继续受 workspace 和 `safe_join` 保护。
- 在迁回 Rust 后，API Key 重新由系统 Keychain 读取，并且不再返回前端。

如果 Anthropic API 因浏览器/WebView 的跨域限制无法直接访问，则增加一个仅用于开发的 Tauri HTTP 转发命令。该命令只负责转发 HTTP 数据，不负责 Agent loop，从而不影响本阶段的学习目标。

---

## 5. 目录规划

```text
src/
├── agent/
│   ├── types.ts              # Message、ContentBlock、ToolCall、Usage 等领域类型
│   ├── events.ts             # AgentEvent 及事件处理接口
│   ├── runner.ts             # Agent loop
│   ├── provider.ts           # LlmProvider 接口
│   ├── anthropic.ts          # 请求构造、SSE 解析、Anthropic 协议转换
│   ├── errors.ts             # 前端 Agent 错误类型
│   └── tools/
│       ├── types.ts          # Tool、ToolContext、ToolResult
│       ├── registry.ts       # 工具注册、schema 输出和执行分发
│       ├── current-time.ts   # 第一项纯前端演示工具
│       ├── memory.ts         # 调用现有 Tauri memory commands
│       └── filesystem.ts     # 调用现有 Tauri fs commands
├── features/
│   ├── chat.tsx              # Chat 页面与 Agent 事件渲染
│   └── settings.tsx          # Provider、Model、临时 API Key 设置
└── server/tauri/             # 保留现有 Rust IPC wrapper
```

不要删除现有 Rust Agent 骨架。前端实现稳定后，再逐个模块对照迁移。

---

## 6. 核心领域模型

首先定义独立于 React 和 Anthropic API 的内部类型，避免 UI、Provider 和 Agent loop 直接耦合。

```ts
export type ChatRole = "user" | "assistant";

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "tool_use"; id: string; name: string; input: unknown }
  | {
      type: "tool_result";
      toolUseId: string;
      content: string;
      isError: boolean;
    };

export interface ChatMessage {
  role: ChatRole;
  content: ContentBlock[];
}

export interface ToolSpec {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}
```

对外事件保持和未来 Rust `AgentEvent` 接近：

```ts
export type AgentEvent =
  | { type: "started"; runId: string }
  | { type: "step_started"; step: number }
  | { type: "text_delta"; delta: string }
  | { type: "text_done"; text: string }
  | { type: "tool_call"; id: string; name: string; input: unknown }
  | {
      type: "tool_result";
      id: string;
      result: unknown;
      isError: boolean;
    }
  | { type: "error"; message: string }
  | { type: "finish"; stopReason: string; usage?: Usage };
```

UI 只消费 `AgentEvent`，不直接理解 Anthropic 的 SSE 事件格式。

---

## 7. 实施顺序

### 7.1 建立可用的聊天界面和运行状态

先把当前占位的 Chat 页面替换为最小聊天界面：

- 消息列表
- 文本输入框
- 发送按钮
- 停止按钮
- 当前运行状态
- 错误提示
- Provider 和 Model 的最小配置

运行状态建议保持简单：

```ts
type RunStatus = "idle" | "running" | "stopping" | "failed";
```

同一时间只允许一个 run，避免多个流同时修改消息。

验收：不接 API，也能在 UI 中提交一条用户消息并显示模拟回复。

### 7.2 实现非流式纯文本请求

先只处理普通文本，不加入工具和循环：

- 设置页输入临时 API Key。
- 选择 Anthropic model。
- `anthropic.ts` 构造 Messages API 请求。
- 将内部 `ChatMessage` 转换为 Anthropic 请求格式。
- 解析完整 JSON 响应。
- 将 assistant 文本显示到聊天页面。
- 对 401、429、网络失败和异常响应给出可理解的错误。

验收：用户发送消息后，可以得到一条完整的模型文本回复。

### 7.3 实现流式文本

在理解非流式请求后，再开启 `stream: true`：

- 使用 `fetch` 获取 `ReadableStream`。
- 按 SSE 空行边界拆分事件，处理跨 chunk 的残留文本。
- 识别 `event:` 和 `data:` 字段。
- 解析 `content_block_delta` 中的 `text_delta`。
- 将 Provider 事件转换为 `AgentEvent.text_delta`。
- UI 将 delta 追加到当前 assistant 消息，而不是每个 delta 新建一条消息。
- 收到结束事件后固化 assistant 消息。

SSE parser 应作为纯函数或独立模块测试，重点覆盖：

- 一个 chunk 包含多个事件
- 一个事件被拆到多个 chunk
- 空行和 `ping`
- 无效 JSON
- 服务端 `error` 事件
- 流正常结束但缺少预期 stop 事件

验收：模型文本逐步显示，且最终内容和完整响应一致。

### 7.4 引入统一 Provider 接口

Agent runner 不直接依赖 Anthropic：

```ts
export interface LlmProvider {
  readonly name: string;

  streamChat(input: LlmCallInput): AsyncIterable<LlmEvent>;
}
```

`LlmEvent` 是 Provider 与 Runner 之间的内部协议，负责表达：

- 文本增量
- 工具调用开始
- 工具参数增量
- 工具调用结束
- 消息结束原因
- Token usage
- Provider 错误

验收：Chat 页面和 Runner 中不出现 Anthropic SSE 的原始字段名。

### 7.5 实现第一个工具

先实现一个无副作用、无需 Tauri 的 `get_current_time`：

```ts
export interface Tool<TInput = unknown, TOutput = unknown> {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
  execute(input: TInput, context: ToolContext): Promise<TOutput>;
}
```

`ToolRegistry` 负责：

- 注册工具
- 输出发给模型的 tool specs
- 按名称查找工具
- 执行工具
- 对未知工具返回结构化错误
- 将异常转换为可反馈给模型的 tool result

不要在 Runner 中写 `if (toolName === ...)`；所有工具必须通过 registry 分发。

验收：询问当前时间时，模型能够发出 tool call，前端执行工具并显示调用参数与结果。

### 7.6 实现完整 Agent loop

Agent loop 是本计划最重要的部分：

```ts
while (step < maxSteps) {
  const turn = await runOneModelTurn(messages, tools);
  messages.push(turn.assistantMessage);

  if (turn.toolCalls.length === 0) {
    finish("end_turn");
    break;
  }

  const results = await executeToolCalls(turn.toolCalls);
  messages.push(createToolResultMessage(results));
}
```

每一轮必须完整保存：

1. assistant 的文本块和 `tool_use` 块。
2. 每个工具的执行结果。
3. 下一轮请求需要的 `tool_result` 块。

边界行为：

- 默认 `maxSteps = 5`，以后可以配置。
- 达到上限时以 `max_steps` 结束，并在 UI 中明确显示。
- 单个工具失败不直接终止 run，而是把错误作为 `tool_result` 交还模型。
- Provider 请求失败终止当前 run。
- 未知工具作为失败的 `tool_result` 返回。
- 第一版按顺序执行多个工具，便于理解和调试；确认语义后再考虑并行。

验收：模型至少能完成“调用时间工具 → 阅读工具结果 → 输出最终答案”的两轮流程。

### 7.7 实现取消和并发保护

每次 run 创建独立的 `AbortController`：

- 停止按钮调用 `abort()`。
- Provider 将 `signal` 交给 `fetch`。
- Runner 在模型调用前、读取流时、工具执行前后检查 signal。
- 取消不是普通错误，UI 显示为“已停止”。
- run 结束后清理 controller，防止旧 run 影响下一次运行。
- 防止用户连续点击发送创建重复 run。

验收：流式输出过程中点击停止，网络读取和 Agent loop 都会停止，并且可以立即开始下一轮对话。

### 7.8 接入现有 Session

继续复用 Rust 已实现的 SQLite session commands，前端负责 Agent，Rust 只负责持久化：

- 新对话时创建 Session。
- 用户消息提交后持久化。
- 每轮 assistant 消息持久化。
- tool use 和 tool result 使用统一序列化格式持久化。
- 切换 Session 时加载并恢复历史消息。
- 新 run 必须使用当前 Session 的历史上下文。
- 消息只有在形成有效内容后才持久化；需要定义取消时是否保存部分文本。

建议第一版规则：取消时保留 UI 中的部分文本，但不将不完整 assistant 消息加入下一轮模型上下文；UI 将其标记为 `aborted`。

验收：重启应用后可以加载历史对话，并基于历史继续交流。

### 7.9 接入 Memory 工具

通过现有 Tauri commands 实现：

- `memory_recall`
- `memory_save`
- 可选的 `memory_update`
- 可选的 `memory_delete`

工具 schema 应限制必要字段，并在描述中写清使用时机。不要在第一版自动把全部 memory 注入 system prompt，先让模型显式调用工具，这样更容易观察 Agent 的决策过程。

验收：模型可以保存一条用户偏好，并在后续会话中主动查询和使用它。

### 7.10 接入 Workspace 文件工具

工具执行仍通过已有 Tauri IPC wrapper：

- `fs_read`
- `fs_write`
- `fs_edit`
- `fs_list_dir`
- `fs_glob`
- `fs_exists`

必须保留 Rust 侧安全边界：

- workspace 未设置时拒绝执行。
- 路径必须经过 `safe_join`。
- 不在前端自行拼接绝对路径绕过 Tauri command。
- 写入和编辑操作在 UI 中清楚展示。
- 输出过长时截断，并告诉模型原始长度。

验收：Agent 可以读取 workspace 中的文件、查找内容并完成一次受控修改，路径穿越仍会被 Rust 拒绝。

### 7.11 完善可观察性和调试体验

聊天界面应能展示一次 run 的真实过程，而不仅是最终答案：

- 当前 step
- 流式文本
- tool name
- tool input
- tool result
- tool error
- stop reason
- usage
- run duration

开发模式可以提供可折叠的原始事件面板，但禁止显示 API Key 和敏感请求头。

验收：仅通过 UI 事件记录，就能解释一次 Agent 为什么调用某个工具、工具返回了什么、为什么继续下一轮以及最终如何停止。

---

## 8. Runner 的职责边界

`runner.ts` 只负责流程编排：

- 管理 messages
- 控制 step
- 消费 Provider 事件
- 组装 assistant message
- 调用 Tool Registry
- 组装 tool result message
- 发出 AgentEvent
- 处理取消、错误和结束

它不应该负责：

- React state
- Anthropic SSE 字段解析
- 具体工具实现
- SQLite 语句
- Tauri `invoke` 的参数细节
- API Key 的持久化

这个边界决定了后续能否顺利将 Runner 翻译成 Rust。

---

## 9. 测试计划

### 单元测试

- 内部 Message 与 Anthropic Message 的转换。
- SSE chunk 拆分与残留缓冲。
- 文本 delta 累积。
- `input_json_delta` 累积及最终 JSON 解析。
- Tool Registry 注册、查找、执行和未知工具。
- 工具异常转换为 `isError: true` 的结果。
- 达到 `maxSteps` 时停止。
- AbortSignal 已取消时不再调用下一轮模型。

### Runner 场景测试

使用 fake provider，不请求真实 API：

1. 一轮纯文本后结束。
2. 第一轮返回工具，第二轮返回最终文本。
3. 一轮返回多个工具。
4. 工具执行失败，模型在下一轮解释错误。
5. Provider 中途失败。
6. 流式输出中途取消。
7. 连续工具调用直到达到步数上限。

### 手动端到端验证

- 临时 Key 刷新后消失。
- 文本流式显示稳定。
- 停止后可以再次发送。
- Session 重载后消息顺序正确。
- Memory 跨 Session 生效。
- Workspace 外路径访问被拒绝。
- 工具调用与结果在 UI 中可见。

---

## 10. 迁回 Rust 的顺序

前端版本稳定后，不一次性重写。按以下顺序逐层迁移，每迁移一层都保持 UI 行为和 `AgentEvent` 不变：

| 顺序 | TypeScript 模块 | Rust 目标模块 | 迁移完成的判断 |
|---|---|---|---|
| 1 | `agent/types.ts` | `llm/mod.rs`、`agent/events.rs` | 两侧消息和事件语义一致 |
| 2 | `agent/anthropic.ts` | `llm/anthropic.rs` | Rust 能产生相同的 Provider 事件 |
| 3 | `agent/tools/*` 接口 | `tools/mod.rs` | Tool spec、执行结果和错误语义一致 |
| 4 | `agent/runner.ts` | `agent/runner.rs` | Rust 可完成相同的多轮 loop |
| 5 | `AbortController` | `CancellationToken` | 停止行为一致 |
| 6 | 前端临时 Key | Rust Keychain | Key 不再进入前端 |
| 7 | 前端事件回调 | Tauri `Channel<AgentEvent>` | UI 无需关心执行位置 |

迁移期间可以保留一个开发开关：

```ts
type AgentRuntime = "frontend" | "rust";
```

同一组 UI 场景分别运行两种实现，用于比较事件顺序、消息格式和最终行为。迁移完成并验证一致后，再删除前端的生产执行入口；前端实现可作为学习参考或测试基准保留。

---

## 11. 暂不实现

为了保持学习路径清晰，本计划暂时不做：

- 多 Provider 支持
- 多 Agent 协作
- 子 Agent
- MCP
- RAG 和向量数据库
- 自动压缩长上下文
- 并行工具调度优化
- 任意 Shell 工具
- 权限确认系统
- 后台长期任务
- 生产级 API Key 前端存储

这些能力应在单 Agent loop、工具调用和上下文流转完全理解之后再设计。

---

## 12. 完成定义

- [ ] 能从 Chat UI 发起一次前端 Agent run。
- [ ] Anthropic 文本可以流式显示。
- [ ] Provider、Runner、Tool、UI 四层边界清晰。
- [ ] Agent 能完成至少一次“模型 → 工具 → 模型”的循环。
- [ ] 支持多个连续步骤并有最大步数保护。
- [ ] 支持停止当前 run。
- [ ] 工具调用、输入、结果和错误都能在 UI 中观察。
- [ ] Session 和消息可以跨重启恢复。
- [ ] Memory 工具可以跨 Session 工作。
- [ ] 文件工具继续受到 Rust workspace 沙箱保护。
- [ ] API Key 不进入源码、Git、日志和数据库。
- [ ] 核心 Runner 有 fake provider 场景测试。
- [ ] 能够逐段说明一次 Agent run 中的消息变化。
- [ ] 前端模块与未来 Rust 模块存在明确迁移对应关系。

完成以上项目后，再开始把 Provider、Tool Registry 和 Agent Runner 按顺序迁回 Rust。
