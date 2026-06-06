# Phase A — UI 脚手架 (Tailwind v4 + shadcn)

> 创建日期: 2026-06-06
> 目标: Tailwind v4 + shadcn/ui 装好,demo `App.tsx` 换成 `AppShell` 空壳。后续每阶段在此基础上加组件。

> **注意**: 全原生架构下前端不需要 `ai` / `@ai-sdk/*` / `zod`,只需要纯 UI 依赖。

## 前置
- `pnpm tauri dev` 跑过,demo 正常

## 步骤

1. **添加纯 UI 依赖**:
   ```bash
   pnpm add -D tailwindcss@^4 @tailwindcss/vite tw-animate-css @types/node vite-tsconfig-paths
   pnpm add clsx tailwind-merge class-variance-authority lucide-react react-markdown remark-gfm rehype-highlight nanoid zustand
   ```

2. **配 Vite + 路径别名**:
   - `vite.config.ts`: 加 `@tailwindcss/vite` plugin、`vite-tsconfig-paths`
   - `tsconfig.json`: `paths` → `@/*` → `src/*`

3. **Tailwind 入口** `src/index.css`:
   ```css
   @import "tailwindcss";
   @import "tw-animate-css";
   @theme { /* tokens 后续加 */ }
   ```
   `src/main.tsx` 顶部 `import "./index.css";`,删 `App.css` 导入。

4. **shadcn 初始化**:
   ```bash
   pnpm dlx shadcn@latest init             # Tailwind v4, alias @/, base slate
   pnpm dlx shadcn@latest add button input textarea dialog sheet card sonner separator tooltip select scroll-area
   ```

5. **替换 App.tsx 为 AppShell 空壳**:
   - 删 demo greet 表单 + logos
   - 新建 `src/components/layout/AppShell.tsx`(左 sidebar 占位 + 右主区占位)
   - 新建 `src/pages/ChatPage.tsx`(放一个 "Chat" 标题)
   - `App.tsx` 内 `<AppShell><ChatPage/></AppShell>`

6. **更新 `index.html`** `<title>Yukin</title>`

## 关键文件

- `vite.config.ts`(改)
- `tsconfig.json`(改)
- `src/index.css`(新)
- `src/App.tsx`(改)
- `src/App.css`(删)
- `src/components/layout/AppShell.tsx`(新)
- `src/pages/ChatPage.tsx`(新)
- `src/components/ui/*`(shadcn 生成)
- `index.html`(改)

## 验证
- [ ] `pnpm tauri dev` 启动
- [ ] 显示 sidebar + main 布局,主区有 "Chat"
- [ ] 控制台无错误
- [ ] 系统 dark/light 切换 UI 跟随
- [ ] 任一 shadcn 组件(如 Button)样式正常

## 风险/陷阱
- Tailwind v4 没有 `tailwind.config.js` 也无 PostCSS;shadcn 要选 v4 流程
- Tauri webview(macOS WebKit / Linux WebKitGTK)对部分新 CSS 特性支持滞后,polyfill 自行处理