# B3 — 模块系统 + `commands/*` 骨架(概念课)

> 创建日期: 2026-06-08
> 配套: [phase B 学习总入口](../2026-06-07-phase-b-learning.md) / [phase B 架构定义](../2026-06-06-phase-b-rust-foundation.md)
> 用途: B3 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

这一步看起来"机械",但藏着 Rust 几个核心概念:

1. Rust 的模块树:`mod` / `use` / 文件布局
2. 可见性修饰符:`pub` / `pub(crate)` / `pub(super)`
3. 参数类型选择:`&str` vs `String` vs `&String`(借用 vs 拥有,**这是 Rust 最反 JS 直觉的一点**)
4. `.into()` 的本质(与 B2 的 `From` 呼应)
5. `#[tauri::command]` 宏帮你做了什么

然后是任务清单 + 自检。

---

## 1. Rust 模块系统(对比 JS)

### JS / TS 的世界

JS 一个文件 = 一个模块,文件路径 = 模块路径,`import` 直接拿其它文件的导出。没有"声明这个文件属于我的项目"这种东西,你 `import` 谁,谁就是模块。

```ts
// utils/path.ts 自动是个模块,只要存在就能 import
import { joinPath } from './utils/path';
```

### Rust 的世界

Rust **不会自动**把一个 `.rs` 文件当成你的模块。你必须显式声明 `mod xxx;` 告诉 crate:"这个文件是我的"。否则编译器根本不看那个文件。

```rust
// lib.rs
mod error;        // ← 告诉 crate: 还有一个文件叫 error.rs,把它挂进来
mod commands;     // ← 告诉 crate: 还有一个模块叫 commands(可能是 commands.rs 或 commands/mod.rs)
```

**两个不同概念**:

| 关键字 | 干什么 | JS 类比 |
|--------|--------|---------|
| `mod xxx;` | **挂载**一个模块进当前 crate(把文件纳入项目) | 没有,JS 自动挂载 |
| `use xxx::Yyy;` | **引入**一个名字到当前作用域(写起来短) | `import { Yyy } from 'xxx'` |

`mod` 是结构性的(只写一次,通常在 `lib.rs` 或 `mod.rs`),`use` 是便利性的(在每个用到的文件里写)。

### 文件布局两种风格

假设你想建模块 `commands`,里面有 `workspace` / `fs` / `keychain` 三个子模块:

**风格 A: `mod.rs` 模式(老式但更常见,我们用这个)**

```
src/
├── lib.rs              # mod commands;
├── commands/
│   ├── mod.rs          # 模块的"入口文件",声明子模块: pub mod workspace; pub mod fs; pub mod keychain;
│   ├── workspace.rs
│   ├── fs.rs
│   └── keychain.rs
```

**风格 B: `commands.rs + commands/` 模式(Rust 2018+ 新式)**

```
src/
├── lib.rs              # mod commands;
├── commands.rs         # 模块的"入口文件",声明子模块
├── commands/
│   ├── workspace.rs
│   ├── fs.rs
│   └── keychain.rs
```

两种等价,Rust 编译器都认。**我们用 A 风格**(phase-b doc 指定),理由:与 `error.rs` 这种叶子模块文件并列时,`commands/mod.rs` 的形式更明显"这是个模块组"。

### `pub mod` vs `mod`

- `mod foo;` —— 挂载 `foo`,**但只在当前模块内可见**(外部 crate 看不到)
- `pub mod foo;` —— 挂载 `foo`,**对外暴露**

对 `commands/mod.rs` 里的子模块:用 `pub mod workspace;` 让 `lib.rs` 能 `use crate::commands::workspace::xxx`。

---

## 2. 可见性修饰符

Rust 默认**一切私有**(包括 struct 字段、函数、模块)。要外部用,显式标 `pub`。

| 修饰符 | 谁能看 | 何时用 |
|--------|--------|--------|
| (默认,无修饰) | 同一个 mod 内部 | 内部 helper 函数 |
| `pub(super)` | 父模块 | 给父模块用但不想散播 |
| `pub(crate)` | 整个 crate 内 | 跨模块的内部 API,不对外发布 |
| `pub` | 完全公开 | crate 的对外 API |

B3 里 `#[tauri::command]` 函数必须 `pub` —— 因为 `generate_handler!` 宏(在 `lib.rs`)需要能引用它们。

`AppError` 也是 `pub`(B2 已经写了)—— 因为命令的 `Err` 类型需要被外部看到。

---

## 3. 参数类型:`&str` vs `String` vs `&String`

**这是 Rust 最反 JS 直觉的部分。** 在 JS,你写函数参数就一个 `string` 完事;在 Rust,你必须想清楚要不要这个字符串的所有权。

### 三种类型

| 类型 | 是什么 | 谁拥有 |
|------|--------|--------|
| `String` | 堆分配的可变字符串,有所有权 | 当前持有者 |
| `&String` | 借用一个 `String` | 借用方临时看一眼 |
| `&str` | 借用一段字符串切片(可以是 `String` 的一部分,也可以是字符串字面量) | 借用方临时看一眼 |

```rust
let s: String = String::from("hello");   // 堆上有一份数据,s 拥有它
let r1: &String = &s;                     // 借用 s 这个 String
let r2: &str = &s;                        // 借用 s 内部的字符串切片(自动解引用)
let lit: &str = "world";                  // 字符串字面量本身就是 &str
```

### 函数参数怎么选?三条铁律

**铁律 1:能借就借,不要拿所有权。**

```rust
// 不好:你函数体如果只是读一下,要走所有权,调用方就被剥夺了 s
fn bad(name: String) { println!("{}", name); }

// 好:借用即可,调用方还能继续用 s
fn good(name: &str) { println!("{}", name); }
```

**铁律 2:参数借用类型用 `&str`,不用 `&String`。**

`&str` 比 `&String` 通用 —— 任何 `&String` 都能自动转 `&str`,但字符串字面量 `"abc"` 是 `&str` 而不是 `&String`。所以参数写 `&str`,调用方传 `String` / 字面量都行。

```rust
fn print_name(name: &str) { ... }

print_name(&String::from("alice"));   // ✓ &String 自动 → &str
print_name("bob");                    // ✓ 字面量本身是 &str
print_name(&some_owned_string);       // ✓
```

**铁律 3:只有当你**真的需要拿走**这个 string(存进 struct、给到别的线程、修改它),才用 `String`。**

```rust
struct User {
    name: String,    // struct 字段必须自己拥有,不能是 &str(除非加 lifetime,后面再学)
}

fn new_user(name: String) -> User {     // 这里必须收 String,因为要存进 struct
    User { name }
}
// 或者
fn new_user(name: &str) -> User {
    User { name: name.to_string() }     // 收借用,函数内 .to_string() 复制一份
}
```

### `.into()` 出场(呼应 B2)

写 todo 占位用的:

```rust
Err(AppError::Other("todo".into()))
```

`"todo"` 是 `&'static str`,`AppError::Other` 要 `String`,需要转。三种写法等价:

```rust
AppError::Other(String::from("todo"))
AppError::Other("todo".to_string())
AppError::Other("todo".into())                     // ← 最简洁
```

`.into()` 调的是 `Into<String> for &str` —— 而这个 trait 是 `impl From<&str> for String` 自动生成的。**这就是 B2 学的 `From` trait 在日常代码里的常见用法**,`?` 是它的另一个用法。

---

## 4. `#[tauri::command]` 宏帮你做了什么

Tauri 命令的本质:**让前端能通过 IPC 调 Rust 函数**。

你写:

```rust
#[tauri::command]
pub async fn get_workspace() -> AppResult<Option<String>> {
    Err(AppError::Other("todo".into()))
}
```

`#[tauri::command]` 这个 proc macro 在编译期把它展开成大约这样(简化):

```rust
pub async fn get_workspace() -> AppResult<Option<String>> {
    Err(AppError::Other("todo".into()))
}

// 加上一个 wrapper,Tauri 的 IPC 层用它做参数反序列化、调你的函数、把结果序列化回去
pub fn __cmd__get_workspace(invoke: tauri::Invoke) {
    // 解析前端传来的 JSON args
    // 调用 get_workspace
    // 把 Result 序列化(Ok 走 Serialize,Err 走 AppError 的 Serialize ← B2 写的)
    // 通过 IPC 回前端
}
```

然后你在 `lib.rs` 写:

```rust
.invoke_handler(tauri::generate_handler![get_workspace, fs_read_text_file, ...])
```

`generate_handler!` 宏生成一个总路由,把前端传的 command 名字 dispatch 到对应的 `__cmd__xxx` wrapper。

**B3 阶段你不需要懂内部,只要知道**:

- 函数前加 `#[tauri::command]` 就能被前端 invoke
- 函数必须 `pub`(`generate_handler!` 引用得到)
- 参数和返回值类型都要支持 `Serialize` / `Deserialize`(基础类型 + 你自己 derive 过的都行)
- 返回 `Result<T, E>` —— `Ok` 在前端是 resolve,`Err` 是 reject

---

## 5. 你的任务

按 phase-b doc 第 5 节列的文件树,**全部新建**。每个文件就是骨架,函数体一律 `Err(AppError::Other("todo".into()))`。

### 文件列表(参考 phase-b doc 第 5 节)

```
src-tauri/src/
├── lib.rs                       # 改: 挂载新模块
├── path_safety.rs               # 新: 函数签名 + unimplemented!()
├── commands/
│   ├── mod.rs                   # 新: pub mod workspace; ...
│   ├── workspace.rs             # 新: get_workspace / set_workspace 等
│   ├── fs.rs                    # 新: fs_read_text_file / fs_write_text_file / fs_glob
│   ├── keychain.rs              # 新: keychain_get / keychain_set / keychain_delete
│   ├── memory.rs                # 新: memory_read / memory_append
│   ├── session.rs               # 新: session_list / session_load / session_create
│   └── agent.rs                 # 新: agent_run / agent_cancel
├── llm/
│   ├── mod.rs                   # 新: LlmProvider trait + ChatMessage / LlmEvent
│   └── anthropic.rs             # 新: struct AnthropicProvider; impl LlmProvider (todo)
├── tools/
│   ├── mod.rs                   # 新: Tool trait + ToolRegistry
│   ├── fs_tool.rs               # 新
│   ├── memory_tool.rs           # 新
│   ├── shell_tool.rs            # 新
│   └── http_tool.rs             # 新
└── agent/
    ├── mod.rs                   # 新: pub mod loop_; pub mod events;  (注意 loop 是关键字!)
    ├── loop_.rs                 # 新: run_agent 函数签名 + unimplemented!()
    └── events.rs                # 新: AgentEvent enum 骨架
```

> ⚠ phase-b doc 写的是 `loop.rs`,但 `loop` 是 Rust 关键字,**不能**当模块名。改用 `loop_.rs`(末尾下划线避关键字,Rust 圈通用约定),或者起别的名字(比如 `runner.rs`)。你来选,记到"实际收获"里。

### 命令具体清单(从 phase-c~g doc 倒推,B3 阶段先列签名)

不需要你现在去查 phase C/D,我这里直接列出 B3 应该建的命令(签名都按"接收什么、返回什么"最朴素的设计):

**`commands/workspace.rs`**
```rust
#[tauri::command]
pub async fn get_workspace() -> AppResult<Option<String>>;
#[tauri::command]
pub async fn set_workspace(path: String) -> AppResult<()>;
#[tauri::command]
pub async fn pick_workspace() -> AppResult<Option<String>>;  // 弹文件夹选择对话框
```

**`commands/fs.rs`**
```rust
#[tauri::command]
pub async fn fs_read_text_file(path: String) -> AppResult<String>;
#[tauri::command]
pub async fn fs_write_text_file(path: String, content: String) -> AppResult<()>;
#[tauri::command]
pub async fn fs_glob(pattern: String) -> AppResult<Vec<String>>;
```

**`commands/keychain.rs`**
```rust
#[tauri::command]
pub async fn keychain_get(service: String, account: String) -> AppResult<Option<String>>;
#[tauri::command]
pub async fn keychain_set(service: String, account: String, secret: String) -> AppResult<()>;
#[tauri::command]
pub async fn keychain_delete(service: String, account: String) -> AppResult<()>;
```

**`commands/memory.rs`**
```rust
#[tauri::command]
pub async fn memory_read() -> AppResult<String>;
#[tauri::command]
pub async fn memory_append(text: String) -> AppResult<()>;
```

**`commands/session.rs`**
```rust
#[tauri::command]
pub async fn session_list() -> AppResult<Vec<String>>;             // 先返回 Vec<String>,后面 Phase E 再改成结构体
#[tauri::command]
pub async fn session_load(id: String) -> AppResult<String>;
#[tauri::command]
pub async fn session_create() -> AppResult<String>;                // 返回新 session id
```

**`commands/agent.rs`**
```rust
#[tauri::command]
pub async fn agent_run(prompt: String) -> AppResult<String>;        // 返回 run_id
#[tauri::command]
pub async fn agent_cancel(run_id: String) -> AppResult<()>;
```

**`path_safety.rs`**
```rust
use std::path::{Path, PathBuf};
use crate::AppResult;

pub fn resolve_within(workspace: &Path, rel: &str) -> AppResult<PathBuf> {
    unimplemented!()    // Phase D 实现
}
```

**`llm/mod.rs`**
```rust
pub mod anthropic;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum LlmEvent {
    Text(String),
    ToolCall { name: String, args: serde_json::Value },
    Done,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: Vec<ChatMessage>) -> AppResult<()>;   // Phase F 改签名为 stream
}
```

**`llm/anthropic.rs`**
```rust
use super::{ChatMessage, LlmProvider};
use crate::AppResult;
use async_trait::async_trait;

pub struct AnthropicProvider {
    // Phase F 加字段(http client、api_key、model 等)
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, _messages: Vec<ChatMessage>) -> AppResult<()> {
        unimplemented!()
    }
}
```

**`tools/mod.rs`**
```rust
pub mod fs_tool;
pub mod memory_tool;
pub mod shell_tool;
pub mod http_tool;

use async_trait::async_trait;
use crate::AppResult;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    async fn call(&self, args: serde_json::Value) -> AppResult<serde_json::Value>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
}
```

**`tools/fs_tool.rs`** 等 4 个:
```rust
use super::Tool;
use crate::AppResult;
use async_trait::async_trait;

pub struct FsTool;

#[async_trait]
impl Tool for FsTool {
    fn name(&self) -> &'static str { "fs" }
    async fn call(&self, _args: serde_json::Value) -> AppResult<serde_json::Value> {
        unimplemented!()
    }
}
```

(其它三个 tool 复制粘贴改名字即可)

**`agent/mod.rs`**
```rust
pub mod events;
pub mod loop_;   // 或你选的别的名
```

**`agent/loop_.rs`**
```rust
use crate::AppResult;

pub async fn run_agent(_prompt: String) -> AppResult<String> {
    unimplemented!()
}
```

**`agent/events.rs`**
```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    Text { content: String },
    ToolCall { name: String },
    Done,
}
```

### 改 `lib.rs`

顶部加全部 mod 挂载:

```rust
mod error;
mod path_safety;
mod state;       // ← 注意! state.rs 是 B4 才建,B3 这里先不加,否则 cargo check 会报找不到文件
mod commands;
mod llm;
mod tools;
mod agent;

pub use error::{AppError, AppResult};
```

`run()` 函数里:
- **暂时保留** `greet` 命令(等 B5 才删,要不然 `generate_handler!` 列表空了)
- B3 阶段**不**改 `generate_handler!`(B5 才统一加新命令)

---

## 6. 验证

```bash
cd src-tauri
cargo check
```

应该通过。可能有一堆 unused warning(`pub` 的东西没人 use)—— 全部忽略,B5/Phase C 起会用上。

`unimplemented!()` 不会让编译失败,它只是"运行到这里 panic"的占位。

---

## 7. 卡点 / 易错点提醒

- **`mod` 必须显式声明** —— 你 `lib.rs` 不写 `mod commands;`,编译器永远看不到 `commands/` 目录
- **子模块要在父模块 `mod.rs` 里再次 `pub mod`** —— `commands/mod.rs` 不写 `pub mod workspace;`,即便 `commands/workspace.rs` 存在,`crate::commands::workspace` 也访问不到
- **`loop` 是 Rust 关键字** —— 文件名/模块名不能叫 `loop`,phase-b doc 写错了,用 `loop_` 或 `runner`
- **`pub mod` 不是 `pub use`** —— 前者挂模块,后者重导出名字。B3 阶段只用 `pub mod`
- **`async_trait`** —— `LlmProvider` / `Tool` 是 trait 带 async 方法,Rust 原生不支持,必须用 `#[async_trait]` 宏(B1 已经装好)
- **`#[serde(tag = "type")]`** —— enum 序列化时给一个 discriminator 字段,前端能 switch。这是常见 pattern
- **`AppResult` 怎么导入** —— 因为 `lib.rs` 写了 `pub use error::{AppError, AppResult}`,模块里直接 `use crate::AppError;` / `use crate::AppResult;` 即可(不用走 `crate::error::AppError`)

---

## 8. 写完贴给我 review 时,我会重点看

- 模块挂载链路是否通(`lib.rs` → `commands/mod.rs` → 每个子文件)
- `pub` / 私有用得对不对
- `&str` vs `String` 选得对不对(看到 `name: &String` 我就要开课)
- `Err(AppError::Other("todo".into()))` 写法统一
- `async_trait` 是否漏了
- `loop` 文件名问题怎么处理的(选了什么 + 原因)
- `cargo check` 输出(warning 数量正常,无 error)
