# Phase F — Anthropic Provider (HTTP + SSE 解析)

> 创建日期: 2026-06-06
> 目标: 实现 `LlmProvider` trait,完成 `llm/anthropic.rs` —— 用 `reqwest` POST Messages API,解析 SSE 流,emit 内部 `LlmEvent`。还没有 agent loop,只是能 stream 出文本。

## 前置
- Phase E 完成(可在 Settings 配 Anthropic key)

## 参考资料
- Anthropic Messages API: `POST https://api.anthropic.com/v1/messages`
- Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- Streaming: 加 `"stream": true`,响应是 SSE,事件类型有:
  - `message_start`, `content_block_start`, `content_block_delta` (内含 `text_delta` 或 `input_json_delta`), `content_block_stop`, `message_delta`, `message_stop`, `ping`, `error`

## 步骤

1. **`src/llm/mod.rs`** 定义共享类型 + trait:
   ```rust
   use async_trait::async_trait;
   use tokio_util::sync::CancellationToken;

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct ChatMessage {
       pub role: ChatRole,              // system|user|assistant|tool
       pub content: Vec<ContentBlock>,
   }
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub enum ContentBlock {
       Text { text: String },
       ToolUse { id: String, name: String, input: serde_json::Value },
       ToolResult { tool_use_id: String, content: String, is_error: bool },
   }

   #[derive(Clone, Debug)]
   pub enum LlmEvent {
       TextDelta(String),
       ToolCallStart { id: String, name: String },
       ToolCallInputDelta { id: String, partial_json: String },
       ToolCallEnd { id: String, input: serde_json::Value },
       MessageStop { stop_reason: String, usage: serde_json::Value },
       Error(String),
   }

   pub struct LlmCallArgs<'a> {
       pub messages: &'a [ChatMessage],
       pub tools: &'a [ToolSpec],       // name + description + input_schema
       pub system: Option<&'a str>,
       pub api_key: &'a str,
       pub model: &'a str,
       pub max_tokens: u32,
       pub sender: tokio::sync::mpsc::UnboundedSender<LlmEvent>,
       pub cancel: CancellationToken,
   }

   #[async_trait]
   pub trait LlmProvider: Send + Sync {
       fn name(&self) -> &str;
       async fn stream_chat(&self, args: LlmCallArgs<'_>) -> AppResult<()>;
   }

   #[derive(Clone, Debug, Serialize)]
   pub struct ToolSpec {
       pub name: String,
       pub description: String,
       pub input_schema: serde_json::Value,
   }
   ```

2. **`src/llm/anthropic.rs`** 实现:
   ```rust
   pub struct Anthropic { client: reqwest::Client }
   const URL: &str = "https://api.anthropic.com/v1/messages";
   const VERSION: &str = "2023-06-01";

   #[async_trait]
   impl LlmProvider for Anthropic {
       fn name(&self) -> &str { "anthropic" }
       async fn stream_chat(&self, args: LlmCallArgs<'_>) -> AppResult<()> {
           let body = json!({
               "model": args.model,
               "max_tokens": args.max_tokens,
               "stream": true,
               "system": args.system,
               "messages": to_anthropic_messages(args.messages),
               "tools": args.tools.iter().map(|t| json!({
                   "name": t.name, "description": t.description, "input_schema": t.input_schema,
               })).collect::<Vec<_>>(),
           });
           let resp = self.client.post(URL)
               .header("x-api-key", args.api_key)
               .header("anthropic-version", VERSION)
               .header("content-type", "application/json")
               .json(&body).send().await?;
           if !resp.status().is_success() {
               let text = resp.text().await.unwrap_or_default();
               return Err(AppError::Llm(format!("HTTP error: {text}")));
           }

           use eventsource_stream::Eventsource;
           use futures::StreamExt;
           let mut stream = resp.bytes_stream().eventsource();
           // 状态: 当前 content_block index → (type, accumulated_json)
           let mut current_tool: Option<(String, String)> = None;  // (id, name)
           let mut current_input_json = String::new();

           while let Some(event) = stream.next().await {
               if args.cancel.is_cancelled() { return Err(AppError::Cancelled); }
               let event = event.map_err(|e| AppError::Llm(e.to_string()))?;
               let data: serde_json::Value = serde_json::from_str(&event.data)?;
               match data["type"].as_str().unwrap_or("") {
                   "content_block_start" => {
                       let block = &data["content_block"];
                       if block["type"] == "tool_use" {
                           let id = block["id"].as_str().unwrap_or("").to_string();
                           let name = block["name"].as_str().unwrap_or("").to_string();
                           current_tool = Some((id.clone(), name.clone()));
                           current_input_json.clear();
                           let _ = args.sender.send(LlmEvent::ToolCallStart { id, name });
                       }
                   }
                   "content_block_delta" => {
                       let delta = &data["delta"];
                       match delta["type"].as_str().unwrap_or("") {
                           "text_delta" => {
                               let _ = args.sender.send(LlmEvent::TextDelta(
                                   delta["text"].as_str().unwrap_or("").to_string()));
                           }
                           "input_json_delta" => {
                               if let Some((id, _)) = &current_tool {
                                   let part = delta["partial_json"].as_str().unwrap_or("");
                                   current_input_json.push_str(part);
                                   let _ = args.sender.send(LlmEvent::ToolCallInputDelta {
                                       id: id.clone(), partial_json: part.to_string() });
                               }
                           }
                           _ => {}
                       }
                   }
                   "content_block_stop" => {
                       if let Some((id, _)) = current_tool.take() {
                           let input: serde_json::Value = serde_json::from_str(&current_input_json)
                               .unwrap_or(json!({}));
                           let _ = args.sender.send(LlmEvent::ToolCallEnd { id, input });
                       }
                   }
                   "message_delta" => { /* delta.stop_reason + usage 累计 */ }
                   "message_stop" => {
                       let _ = args.sender.send(LlmEvent::MessageStop {
                           stop_reason: "end_turn".into(),
                           usage: json!({}),
                       });
                   }
                   "error" => {
                       let _ = args.sender.send(LlmEvent::Error(data["error"]["message"].as_str().unwrap_or("unknown").to_string()));
                   }
                   _ => {}  // ping, message_start, etc.
               }
           }
           Ok(())
       }
   }
   ```

3. **临时测试命令** `chat_test`(放 `commands/agent.rs`,**Phase G 删除**):
   ```rust
   #[tauri::command]
   pub async fn chat_test(prompt: String, model: String, channel: tauri::ipc::Channel<AgentEvent>, state: State<'_, AppState>) -> AppResult<()> {
       let key = crate::commands::keychain::internal_get_key("anthropic").await?
           .ok_or(AppError::Other("no api key".into()))?;
       let provider = Anthropic { client: state.http.clone() };
       let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
       let cancel = CancellationToken::new();
       let messages = vec![ChatMessage::user_text(&prompt)];

       // spawn LLM call
       let call = tokio::spawn(async move {
           provider.stream_chat(LlmCallArgs {
               messages: &messages, tools: &[], system: None,
               api_key: &key, model: &model, max_tokens: 1024,
               sender: tx, cancel,
           }).await
       });

       // forward LlmEvent → AgentEvent → channel
       while let Some(evt) = rx.recv().await {
           let agent_evt = match evt {
               LlmEvent::TextDelta(d) => AgentEvent::TextDelta { delta: d },
               LlmEvent::MessageStop {..} => AgentEvent::Finish { usage: json!({}) },
               LlmEvent::Error(m) => AgentEvent::Error { message: m },
               _ => continue,
           };
           let _ = channel.send(agent_evt);
       }
       call.await.map_err(|e| AppError::Other(e.to_string()))??;
       Ok(())
   }
   ```

4. **前端临时调用**: 在 `ChatPage` 加按钮:
   ```ts
   const channel = new Channel<AgentEvent>();
   channel.onmessage = (e) => { if (e.type === "TextDelta") appendToFrame(e.delta); };
   await invoke("chat_test", { prompt: "say hi", model: "claude-sonnet-4-6", channel });
   ```

## 关键文件
- `src-tauri/src/llm/mod.rs`(完整 trait + types)
- `src-tauri/src/llm/anthropic.rs`(完整实现)
- `src-tauri/src/commands/agent.rs`(临时 `chat_test`)
- `src/pages/ChatPage.tsx`(临时按钮触发)

## 验证
- [ ] 设好 anthropic key
- [ ] 点 "Send hi",UI 出现流式追加的 "Hi! How can I help you today?" 或类似
- [ ] 控制台无错误
- [ ] 故意把 key 设错,看到 LLM 错误事件 → UI 显示错误
- [ ] 切换 model(如 `claude-haiku-4-5-20251001`)流式仍正常

## 风险/陷阱
- Anthropic 的 `tool_use` block 的 `input` 是流式拼接的 JSON 串,要等 `content_block_stop` 才完整可解析(`input_json_delta` 的 partial 不是合法 JSON)
- SSE 的 `event: ` 行可被忽略,只看 `data:`(`eventsource-stream` 自动处理)
- API key 头不能写错: `x-api-key`(不是 `Authorization`),`anthropic-version` 必须有
- `max_tokens` 必填,给 4096 比较安全
- `keychain::internal_get_key` 是 Rust 内部函数(不 expose 给前端),命名清晰避免误注册到 `generate_handler!`