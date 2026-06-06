# Phase G — Agent Loop + Tool Trait + Channel 推送

> 创建日期: 2026-06-06
> 目标: 把 `tools::Tool` trait 定型,搭好 `ToolRegistry`,实现 `agent::loop::run_agent` —— 接 LLM stream → 遇到 tool_use → 调 registry → 把结果作为 `tool_result` 喂回 LLM → 继续直到 `stop_reason=end_turn` 或步数上限。

## 前置
- Phase F 完成(Anthropic provider 可流式)

## 步骤

1. **`src/agent/events.rs`** — 前端唯一关心的事件:
   ```rust
   #[derive(Clone, Debug, Serialize)]
   #[serde(tag = "type")]
   pub enum AgentEvent {
       TextDelta { delta: String },
       TextDone,
       ToolCall { id: String, name: String, input: serde_json::Value },
       ToolResult { id: String, result: serde_json::Value, is_error: bool },
       Error { message: String },
       Finish { stop_reason: String, usage: serde_json::Value },
       Started { run_id: String },
   }
   ```

2. **`src/tools/mod.rs`** — Tool trait + registry:
   ```rust
   use async_trait::async_trait;
   use schemars::{JsonSchema, schema_for};

   #[async_trait]
   pub trait Tool: Send + Sync {
       fn name(&self) -> &str;
       fn description(&self) -> &str;
       fn input_schema(&self) -> serde_json::Value;
       async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> AppResult<serde_json::Value>;
   }

   pub struct ToolContext<'a> {
       pub state: &'a AppState,
       pub cancel: &'a CancellationToken,
       pub session_id: &'a str,
   }

   pub struct ToolRegistry { tools: Vec<Box<dyn Tool>> }
   impl ToolRegistry {
       pub fn new() -> Self { Self { tools: vec![] } }
       pub fn register(&mut self, t: Box<dyn Tool>) { self.tools.push(t); }
       pub fn specs(&self) -> Vec<ToolSpec> { /* 转 LLM 看的 spec */ }
       pub async fn execute(&self, name: &str, input: serde_json::Value, ctx: &ToolContext) -> AppResult<serde_json::Value> {
           let t = self.tools.iter().find(|t| t.name() == name)
               .ok_or(AppError::Other(format!("unknown tool: {name}")))?;
           t.execute(input, ctx).await
       }
   }
   ```

   Helper 宏 / pattern 让 tool 简化:
   ```rust
   #[derive(Deserialize, JsonSchema)]
   pub struct FsReadInput { pub path: String }

   pub struct FsReadTool;
   #[async_trait]
   impl Tool for FsReadTool {
       fn name(&self) -> &str { "fs_read" }
       fn description(&self) -> &str { "Read a UTF-8 text file from the workspace." }
       fn input_schema(&self) -> serde_json::Value {
           serde_json::to_value(schema_for!(FsReadInput)).unwrap()
       }
       async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> AppResult<serde_json::Value> {
           let i: FsReadInput = serde_json::from_value(input)?;
           // 调 Phase D 的 fs_read 内部函数(不是 #[tauri::command])
           let r = crate::commands::fs::internal_fs_read(&i.path, ctx.state).await?;
           Ok(serde_json::to_value(r)?)
       }
   }
   ```

   > Phase H 实现所有 tool;G 阶段先注册一个 dummy tool(返回 `{"ok":true}`)用于 loop 联调。

3. **`src/agent/loop.rs`** — 核心 loop:
   ```rust
   pub async fn run_agent(
       provider: Arc<dyn LlmProvider>,
       registry: Arc<ToolRegistry>,
       state: Arc<AppState>,
       session_id: String,
       initial_messages: Vec<ChatMessage>,
       system: Option<String>,
       model: String,
       api_key: String,
       channel: tauri::ipc::Channel<AgentEvent>,
       cancel: CancellationToken,
   ) -> AppResult<()> {
       let mut messages = initial_messages;
       let tools = registry.specs();
       let mut step = 0;
       const MAX_STEPS: usize = 12;

       loop {
           if cancel.is_cancelled() { return Err(AppError::Cancelled); }
           if step >= MAX_STEPS {
               let _ = channel.send(AgentEvent::Finish { stop_reason: "max_steps".into(), usage: json!({}) });
               return Ok(());
           }
           step += 1;

           // 一次 LLM call
           let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<LlmEvent>();
           let call_cancel = cancel.child_token();
           let call_messages = messages.clone();
           let call_tools = tools.clone();
           let call_system = system.clone();
           let call_key = api_key.clone();
           let call_model = model.clone();
           let call_provider = provider.clone();
           let call_handle = tokio::spawn(async move {
               call_provider.stream_chat(LlmCallArgs {
                   messages: &call_messages, tools: &call_tools,
                   system: call_system.as_deref(),
                   api_key: &call_key, model: &call_model, max_tokens: 4096,
                   sender: tx, cancel: call_cancel,
               }).await
           });

           // 累积此轮 assistant 输出
           let mut text_buf = String::new();
           let mut pending_tools: Vec<(String, String, serde_json::Value)> = vec![];

           while let Some(evt) = rx.recv().await {
               if cancel.is_cancelled() { break; }
               match evt {
                   LlmEvent::TextDelta(d) => {
                       text_buf.push_str(&d);
                       let _ = channel.send(AgentEvent::TextDelta { delta: d });
                   }
                   LlmEvent::ToolCallEnd { id, input } => {
                       // ToolCallStart 时 LLM 还不知 name(其实知道,见 Phase F 修正)→ name 应在 Start 时缓存
                       // 此处假设我们在 LlmEvent::ToolCallStart 把 name 也保存
                       let name = "...";  // 见下方 fix
                       pending_tools.push((id.clone(), name.into(), input.clone()));
                       let _ = channel.send(AgentEvent::ToolCall { id, name: name.into(), input });
                   }
                   LlmEvent::MessageStop { stop_reason, usage } => {
                       if !text_buf.is_empty() { let _ = channel.send(AgentEvent::TextDone); }
                       call_handle.await.map_err(|e| AppError::Other(e.to_string()))??;

                       // 持久化 assistant 消息
                       let assistant_msg = build_assistant_message(&text_buf, &pending_tools);
                       persist_message(&state.db, &session_id, &assistant_msg).await?;
                       messages.push(assistant_msg);

                       if pending_tools.is_empty() {
                           let _ = channel.send(AgentEvent::Finish { stop_reason, usage });
                           return Ok(());
                       }

                       // 执行所有 tool calls,收集结果
                       let mut tool_results = vec![];
                       for (id, name, input) in &pending_tools {
                           let ctx = ToolContext { state: &state, cancel: &cancel, session_id: &session_id };
                           let (result, is_error) = match registry.execute(name, input.clone(), &ctx).await {
                               Ok(r) => (r, false),
                               Err(e) => (json!({ "error": e.to_string() }), true),
                           };
                           let _ = channel.send(AgentEvent::ToolResult { id: id.clone(), result: result.clone(), is_error });
                           tool_results.push((id.clone(), result, is_error));
                       }

                       // 把 tool_result 作为 user 消息加入,继续 loop
                       let tool_msg = build_tool_result_message(&tool_results);
                       persist_message(&state.db, &session_id, &tool_msg).await?;
                       messages.push(tool_msg);
                       break;  // 跳出 inner while,进下一轮 LLM call
                   }
                   LlmEvent::Error(m) => {
                       let _ = channel.send(AgentEvent::Error { message: m.clone() });
                       return Err(AppError::Llm(m));
                   }
                   _ => {}
               }
           }
       }
   }
   ```

   > **修正**: `LlmEvent::ToolCallStart` 已带 `name`(见 Phase F),loop 内部维护 `HashMap<id, name>` 用于配对。

4. **`commands/agent.rs`** — 把临时的 `chat_test` 删掉,实现真命令:
   ```rust
   #[tauri::command]
   pub async fn chat_send(
       session_id: String,
       content: String,
       channel: tauri::ipc::Channel<AgentEvent>,
       state: State<'_, AppState>,
   ) -> AppResult<String> {
       let run_id = Uuid::new_v4().to_string();
       let cancel = CancellationToken::new();
       state.runs.write().await.insert(run_id.clone(), cancel.clone());
       let _ = channel.send(AgentEvent::Started { run_id: run_id.clone() });

       // 持久化 user 消息
       let user_msg = ChatMessage::user_text(&content);
       persist_message(&state.db, &session_id, &user_msg).await?;

       // 加载该会话所有历史消息
       let history = load_session_messages(&state.db, &session_id).await?;

       // 拼 system prompt(workspace + Phase I 加 memory)
       let system = build_system_prompt(&state).await;

       // 构造 provider + registry(暂时只注册 dummy)
       let provider: Arc<dyn LlmProvider> = Arc::new(Anthropic { client: state.http.clone() });
       let registry = build_tool_registry();  // Phase H 实装

       let key = internal_get_key("anthropic").await?.ok_or(AppError::Other("no key".into()))?;
       let model = read_settings(&state.db, "selected_model").await?
           .unwrap_or_else(|| "claude-sonnet-4-6".into());

       // 后台执行,不阻塞 invoke 返回
       let session_id2 = session_id.clone();
       let state2 = state.inner().clone();   // AppState 要 Arc 化或 Clone-able
       tokio::spawn(async move {
           let _ = run_agent(provider, Arc::new(registry), state2,
                             session_id2, history, Some(system),
                             model, key, channel, cancel).await;
       });
       Ok(run_id)
   }

   #[tauri::command]
   pub async fn chat_abort(run_id: String, state: State<'_, AppState>) -> AppResult<()> {
       if let Some(token) = state.runs.write().await.remove(&run_id) {
           token.cancel();
       }
       Ok(())
   }
   ```

5. **Dummy tool 注册**(G 阶段联调用):
   ```rust
   fn build_tool_registry() -> ToolRegistry {
       let mut r = ToolRegistry::new();
       r.register(Box::new(EchoTool));   // 返回输入参数本身
       r
   }
   ```

6. **前端 `ChatPage`** 升级:
   - 删除临时按钮,改为 Composer 提交流程
   - `tauri.agent.send(sessionId, content, onEvent)`
   - onEvent: 根据类型追加到 message frame 或创建 ToolCallView 卡片
   - 保存 `currentRunId`,Stop 按钮 → `tauri.agent.abort(runId)`

## 关键文件
- `src-tauri/src/agent/events.rs`(完整)
- `src-tauri/src/agent/loop.rs`(完整 run_agent)
- `src-tauri/src/tools/mod.rs`(完整 trait + registry)
- `src-tauri/src/commands/agent.rs`(`chat_send` + `chat_abort`)
- `src-tauri/src/llm/anthropic.rs`(修正: ToolCallStart 带 name)
- `src-tauri/src/state.rs`(AppState 改 `Arc` 或加 Clone 字段)
- `src/components/chat/{ToolCallView,Composer,MessageItem}.tsx`(实装)
- `src/pages/ChatPage.tsx`(完整流程)

## 验证
- [ ] 配好 key,Composer 提交 "say hi" → UI 流式渲染,Channel 通畅
- [ ] 提交 "please call the echo tool with {a:1}" → 看到 ToolCall + ToolResult 卡片
- [ ] Stop 按钮按下,~200ms 内 stream 停止,abort 命令成功
- [ ] DB Browser 验证 messages 表内 user / assistant 两条消息按顺序写入
- [ ] 故意把 echo tool execute 抛错 → UI 显示 is_error 状态

## 风险/陷阱
- `AppState` 现在要被 `tokio::spawn` 持有 → 改成 `Arc<AppState>` 或所有字段都内含 `Arc/Pool`(sqlx Pool 已是 Arc 包装)
- `Channel<T>` 在 spawn 后能否安全跨任务?可,它是 `Send + Clone`
- Tool 错误返回(`is_error: true`)需变成对 LLM 的 `tool_result` 块的 `is_error: true` 字段,模型才会知道工具失败并修正