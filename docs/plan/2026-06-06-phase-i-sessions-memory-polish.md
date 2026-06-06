# Phase I — 会话管理 + Memory Recall 注入 + UX 打磨

> 创建日期: 2026-06-06
> 目标: 多会话侧边栏,系统提示自动注入相关 memory,首次 shell 确认,abort 完整链路,代码块复制,错误 toast。**v1 完结线**。

## 前置
- Phase H 完成(所有 tool 跑通)

## 步骤

1. **`SessionSidebar.tsx`**:
   - 顶部 "New chat" 按钮 → `tauri.session.create("Untitled")`
   - 列表项: 标题 + 时间;点击 switch
   - 右键菜单(用 shadcn `ContextMenu`): 重命名 / 删除
   - 重命名: 弹 Dialog input → `tauri.session.update(id, {title})`
   - 删除: AlertDialog 确认 → `tauri.session.delete(id)`(FK cascade 清 messages)
   - 当前会话高亮(读 `sessions.currentId`)

2. **会话切换流程**:
   ```ts
   async function switchSession(id: string) {
     const msgs = await tauri.session.loadMessages(id);
     setMessages(msgs.map(persistedToFrame));   // 重建 text 与 ToolCallView 卡片
     setCurrentId(id);
   }
   ```
   `persistedToFrame`: role/content/tool_calls/tool_results → UI frame 数组。

3. **System prompt 注入相关 memory**:
   - 修改 `build_system_prompt(state, last_user_message?)`:
     ```rust
     pub async fn build_system_prompt(state: &AppState, recent_query: Option<&str>) -> String {
         let mut s = base_prompt(state).await;
         if let Some(q) = recent_query {
             let mems = sqlx::query_as::<_, MemoryRow>(
                 "SELECT m.* FROM memory m JOIN memory_fts f ON f.rowid=m.rowid
                  WHERE memory_fts MATCH ?1 ORDER BY rank LIMIT 6"
             ).bind(q).fetch_all(&state.db).await.unwrap_or_default();
             if !mems.is_empty() {
                 s.push_str("\n\n## Relevant memories (from past sessions)\n");
                 for m in mems {
                     let content = m.content.chars().take(300).collect::<String>();
                     s.push_str(&format!("- [{}] {}: {}\n", m.kind, m.name, content));
                 }
             }
         }
         s
     }
     ```
   - `chat_send` 调用时传入用户消息作为 query。

4. **首次 shell 确认 UI**(会话级):
   - zustand `ui.shellAuthorizedSessions: Set<string>`
   - 当 ToolCall 名为 `shell_exec` 且 sessionId 不在 set 内时:
     - 在 ToolCallView 上方渲染横幅: "Agent wants to run shell commands in this session."
     - 按钮: [Allow this session] / [Deny once]
     - Allow → 加入 set + 让 tool 继续(其实 tool 已经被执行,这里其实是 *阻塞前置* 设计,需要 Rust 侧支持: 在 ToolCall 事件后等待前端确认才执行)
   - **简化方案(v1)**: 只是 *提示*,不阻塞 —— 首次出现 shell_exec 时弹 toast: "Agent ran a shell command. Settings → Permissions to manage."。完整阻塞确认推迟 v2。

5. **Abort 完整链路**:
   - 已在 Phase G 实现 `chat_abort(run_id)`
   - 确认在 Phase H 的 shell_exec 中 cancel 时 `child.kill().await`
   - Composer 在 `Started` 事件后启用 Stop 按钮,在 `Finish/Error` 后禁用
   - Stop 按下:`tauri.agent.abort(runId)` + 本地立刻把当前消息 frame 标记 "(aborted)"

6. **代码块复制**:
   - `Markdown.tsx` 自定义 `code` block 渲染:
     - 渲染 `<pre><code class="language-xxx">`
     - 右上角浮 "Copy" button → `navigator.clipboard.writeText(rawCode)`
     - 已配 `rehype-highlight` 做高亮

7. **错误 toast**:
   - `lib/tauri.ts` 的每个 invoke 包装 try-catch → 抛 `AppError`(`{code, message}` 形)
   - 全局 `Toaster`(shadcn `sonner`)在 `App.tsx`
   - ChatPage 的 onEvent 收到 `Error` → `toast.error(e.message)`
   - 其他随机错误也用 toast 反馈

8. **UI 小优化**:
   - 消息列表自动滚到底(stream 中)
   - 输入框 Enter 发,Shift+Enter 换行
   - Settings 页加 "Reset all memories" 危险按钮(AlertDialog 二次确认)
   - 状态指示器:左下角显示当前 workspace 短路径 + provider/model

## 关键文件
- `src/components/layout/SessionSidebar.tsx`(新)
- `src/lib/store/sessions.ts`(扩 update/delete/switch + loadMessages)
- `src-tauri/src/agent/prompts.rs`(改 build_system_prompt)
- `src-tauri/src/commands/agent.rs`(chat_send 传 user query 给 build_system_prompt)
- `src/components/chat/{Markdown,Composer,ToolCallView}.tsx`(代码块复制、Stop、shell 提示)
- `src/lib/store/ui.ts`(shellAuthorizedSessions)
- `src/pages/SettingsPage.tsx`(Reset memories)
- `src/App.tsx`(<Toaster/>)

## 验证
- [ ] 新建 3 个会话,自由切换,消息隔离
- [ ] 会话 A: "remember I prefer pnpm" → memory_save → 看到 DB 行
- [ ] 会话 B: "what package manager do I prefer?" 
   - 看系统 prompt(可在 Rust tracing 日志确认)包含 "Relevant memories: [user] pnpm preference"
   - 或 agent 自主 memory_recall 命中
- [ ] 删除会话 → 列表消失,messages 表行被 cascade 删
- [ ] 重启 app,会话 + memory 全在
- [ ] shell_exec 调用后弹 toast 提示
- [ ] stream 中点 Stop → 立刻停 + 当前 message frame 标 "(aborted)"
- [ ] 故意填错 key 发消息 → 红 toast,UI 不卡死
- [ ] 代码块右上角 Copy 按钮可用
- [ ] Settings 点 "Reset all memories" → 确认 → memory 表清空

## 风险/陷阱
- shell 沙箱仍是软的;v1 用 toast 提示,v2 改阻塞确认(Rust 侧 emit `AwaitToolConfirm` → 前端确认 → emit `ConfirmToolResult` → Rust 继续 execute)
- abort 后 in-flight tool 已加 cancel select,但 LLM HTTP 请求只在下一次 loop iteration 才检查 cancel —— 在 stream_chat 内部也要 poll cancel(`tokio::select!`)
- memory_recall 注入可能噪音多 → 限 6 条,每条截 300 字符,合计 <2KB 影响 prompt 不大
- FTS5 中文场景: 若 user 提示是中文,memory 命中差,后续可加 trigram tokenizer 或 embedding 检索(schema 已预留 `ALTER TABLE memory ADD COLUMN embedding BLOB`)

## v1 完结

通过 Phase I 验证 = Yukin Agent v1.0 完成:
- ✅ Anthropic Claude 直连(Key 永不出 Rust 进程)
- ✅ CSP 锁死 `'self' ipc:`,无外网通信表面
- ✅ 9 种 tool: 5 fs + 2 memory + shell + http
- ✅ 多会话 + 跨会话 memory
- ✅ Abort + 数据持久化 + 错误反馈完整
- ✅ Workspace 沙箱化文件操作
- ✅ 整套架构原生 Rust,可扩展任意 provider/tool