# 设计修正记录 (Design Patch)

> 本文件记录在开发前讨论中对原始计划(2026-06-06-*.md)做出的修正。
> 原始文档保留不动,作为第一次设计快照;
> 本 patch 记录实际开发时的真实决策,两条线索并进。

---

## P1. 路由: zustand currentView → MemoryRouter

**触发点**: 讨论"UI 是否要路由"时提到 react-router 的 MemoryRouter。

**决策**:
- 使用 `react-router-dom` 的 `<MemoryRouter>`(不操作 URL)
- 移除 plan 中所有预先指定的 **zustand router/store** 部分
- URL 结构仅此一阶:
  ```
  /                        → redirect /chat/new (或 last session)
  /chat/:sessionId         → ChatPage (useParams)
  /chat/new                → 新会话
  /settings                → SettingsPage (暂不嵌套子页)
  ```

**收益**:
- 会话切换 = `navigate('/chat/' + id)`,ChatPage 内 `useParams` 监听 id 变化自动 reload messages
- 无需全局 store 管理 currentView / currentSessionId
- 流式中的临时 message frames 用 ChatPage 内 `useState` 持有,切换会话时自然卸载(=防止跨会话状态泄露)

**波及范围**:
| 文件 | 改动 |
|------|------|
| phase-a-ui-scaffold.md | 依赖: **删除 zustand, 加 react-router-dom** |
| phase-e-frontend-stores-settings.md | **整篇重写**(见 P3) |
| phase-i-sessions-memory-polish.md | 会话切换从 store action 改成 `navigate()` |
| README.md | 架构图: 状态流向修正 |

---

## P2. 状态管理: zustand stores → "需要时再加"

**触发点**: 提出"所有持久化在 Rust,前端不存在数据层"。

**决策**:
- 不为 zustand 预留任何 store 目录和空文件
- 跨组件 UI 状态先用 `React Context` 顶
- 真的发现 Context boilerplate > zustand 开销时再加,但**不预先计划**
- 目前能看到的"需要全局共享"的状态基本不存在

**具体替代方案**:
| 原 plan 写的 store | 实作用什么 |
|--------------------|-----------|
| workspace store | `tauri.workspace.get()` 直接在 Settings 页 `useEffect` 拉;ChatPage 从系统 prompt 得知 |
| settings store | Settings 页内 `useState`(配完后同步到 SQLite,重启时读取) |
| sessions store | **MemoryRouter 接管**: session list → API call 渲染,切换 → navigate |
| ui store | sidebar 开合 → `useState` in AppShell; toasts → sonner 内置 |

**波及范围**:
| 文件 | 改动 |
|------|------|
| phase-a-ui-scaffold.md | 删 `src/lib/store/` 目录计划 |
| phase-e-frontend-stores-settings.md | 整篇重写 |
| phase-i-sessions-memory-polish.md | 无 store 可改,直接调用 API |

---

## P3. Phase E 重写合并为 "Frontend Wiring + Settings"

**原标题**: `phase-e-frontend-stores-settings.md`
**新功能**: `phase-e-frontend-wiring-settings.md`(实际执行时覆盖)

**变化**:
1. 删除 `src/lib/store/` 四个文件;
2. 删除 `zustand persist` 相关描述;
3. `tauri.ts` 还是原来的 typed invoke 包装;
4. 加 MemoryRouter 初始化 + Layout 路由配置;
5. ChatPage 使用 `useParams<{ sessionId: string }>()` 接收会话 id;
6. Settings API key 页面用 `useState` 管理输入框状态,提交后直接调 `tauri.key.set`;
7. 会话列表仍然是一个 `<aside>` 组件从 API 拉数据渲染,点击触发 `navigate()`。

---

## P4. Phase A 依赖确认

**npm 依赖** (修正后):
```bash
pnpm add -D tailwindcss@^4 @tailwindcss/vite tw-animate-css @types/node vite-tsconfig-paths
pnpm add clsx tailwind-merge class-variance-authority lucide-react
pnpm add react-markdown remark-gfm rehype-highlight
pnpm add nanoid
pnpm add react-router-dom          # 替代 zustand
```

**不装**:
- ~~zustand~~(需要时再加)
- ~~ai / @ai-sdk/*~~(全 Rust,前端不调 AI)
- ~~@tauri-apps/plugin-sql~~(SQL 只通过 Rust commands,前端不直接连)

**Rust Cargo 依赖** (不变,原计划准确):
- 仍含 `tauri-plugin-sql`(Rust 代码自己用 sqlx 直连,`tauri-plugin-sql` 只用作 Tauri 插件注册的桩,前端不调用)

---

## 修正总结

| 变更点 | 前(plan 文档) | 后(实际执行) |
|--------|--------------|-------------|
| 页面切换 | zustand store currentView | MemoryRouter `<Routes>` |
| 会话 id | zustand store currentId | `useParams().sessionId` |
| 全局状态 | 4 个 zustand stores | 不预先设置,React 自带,需要时再加 |
| 持久化策略 | zustand persist → SQLite | 全在 Rust,前端只 invoke |
| 流式 frame | zustand sessions store | ChatPage `useState` |
| 设置页数据 | zustand settings store + refresh | `useState` 本地,提交时 invoke 落 Rust |
| 依赖 | `zustand` | `react-router-dom` |