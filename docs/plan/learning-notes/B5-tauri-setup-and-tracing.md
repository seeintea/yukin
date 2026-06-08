# B5 — `lib.rs`:Tauri setup hook + `tracing` + plugin 注册(概念课)

> 创建日期: 2026-06-08
> 配套: [phase B 学习总入口](../2026-06-07-phase-b-learning.md) / [phase B 架构定义](../2026-06-06-phase-b-rust-foundation.md)
> 用途: B5 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

这一步是学习重头戏 3 —— **async / Tauri 接缝**。看似只是"写 lib.rs",实际是把 B2-B4 的所有积累(错误 / 模块 / 状态)第一次**串起来跑**,中间会撞上几个真实工程才会碰到的问题。

分 9 节:

1. Tauri 程序的启动流程总览
2. Builder pattern:为什么 Tauri 用一连串 `.plugin().setup().invoke_handler()`
3. plugin 注册:做了什么、顺序有没有讲究
4. `setup` hook 签名:**为什么 closure 返回 `Result<_, Box<dyn Error>>` 而不是 `AppResult`**
5. **`block_on` 在 sync setup 里调 async `AppState::new`(本节最大坑)**
6. `app.manage(state)`:TypeMap 是怎么按类型查找的
7. `generate_handler!` 宏:编译期生成 IPC 路由表
8. `tracing` vs `println!` / `log`:结构化日志
9. `RUST_LOG=yukin=debug` 如何让 EnvFilter 生效

然后是任务、验证、卡点提醒。

---

## 1. Tauri 程序启动流程总览

走完一次完整启动,内部发生了这些事(简化版):

```
main()                                  ← src-tauri/src/main.rs 一行调 lib::run()
  └─ run()                              ← src-tauri/src/lib.rs(我们要改的就是这个)
       ├─ Builder::default()            ← 造一个空的 app builder
       │
       ├─ .plugin(opener::init())       ← 装 opener 插件(注册 IPC 命令、配 capability)
       ├─ .plugin(dialog::init())       ← 装 dialog 插件
       ├─ .plugin(sql::Builder::default().build())  ← 装 sql 插件
       │
       ├─ .setup(|app| {                ← 启动时的一次性初始化 hook
       │      // 在这里:初始化 tracing,造 AppState,app.manage(state)
       │      Ok(())
       │  })
       │
       ├─ .invoke_handler(generate_handler![ ... ])  ← 注册所有 #[tauri::command] 函数
       │
       └─ .run(generate_context!())     ← 启动事件循环、打开主窗口、阻塞等待退出
```

`.run(...)` 调用之前所有 `.xxx()` 都只是**配置**(往 Builder 里塞数据),`.run(...)` 真正点火。Builder pattern 的好处是配置可读、可选项任意组合、编译期类型完整。

---

## 2. Builder pattern

JS 里你会看到:

```js
const app = createApp({ plugins: [a, b], setup: fn, handlers: { ... } });
```

一个大 object,字段名和值都来自调用方。

Rust 里**没有可选命名参数**(故意的语言决策),要表达"一堆可选配置 + 顺序无关 + 类型安全",最常见的就是 Builder pattern:每个 `.xxx()` 方法消费 `self`、返回新的 `Self`,链下去就把配置堆起来。

```rust
let app = tauri::Builder::default()
    .plugin(p1)              // 返回 Builder
    .plugin(p2)              // 返回 Builder
    .setup(closure)          // 返回 Builder
    .invoke_handler(...)     // 返回 Builder
    .build(context)?;        // 消耗 Builder,返回 App
```

每一步都是**编译期类型安全**:`generate_handler!` 展开的类型签名必须匹配 `.invoke_handler` 的参数,错了不能编译。这就是 Rust 不需要 runtime schema validation 的根本原因。

---

## 3. plugin 注册

`tauri_plugin_xxx::init()` 返回一个实现了 `tauri::plugin::Plugin` trait 的对象。`.plugin(p)` 做的事:

1. 收集这个 plugin 提供的 IPC 命令(比如 dialog 提供 `plugin:dialog|open` 这种命名)
2. 收集 plugin 自己的 setup 逻辑(如果有,会在 app setup 之后跑)
3. 把 plugin 实例存进 Builder,等 `.run()` 时一起 wire 进 app

### 注册顺序有没有讲究?

99% 情况**无所谓**。例外:

- 如果两个 plugin 注册了同名 IPC 命令,**后注册的覆盖先注册的**(很罕见)
- 如果 plugin B 的 setup 依赖 plugin A 已经初始化好,要按 A → B 注册
- 我们用的 `opener` / `dialog` / `sql` 三个互相独立,顺序无所谓

实践习惯是**字母序**或**按重要性**(核心插件先,辅助插件后),便于以后 review。

### 我们 B5 要注册哪几个

按 [phase-b-rust-foundation](../2026-06-06-phase-b-rust-foundation.md) §1.2 / §1.3:

```rust
.plugin(tauri_plugin_opener::init())          // 已经在 demo 里有了,保留
.plugin(tauri_plugin_dialog::init())          // workspace 选目录用
.plugin(tauri_plugin_sql::Builder::default().build())  // Phase C 真正用,这里先挂上
```

注意 `sql` 是 `Builder::default().build()` 而不是 `init()` —— 因为它需要更复杂的配置(后续可以加 migrations 等)。其它两个是 zero-config 直接 `init()`。

---

## 4. `setup` hook 签名(为什么不返回 `AppResult`)

签名长这样:

```rust
.setup(|app: &mut App| -> Result<(), Box<dyn std::error::Error>> {
    // 你的初始化代码
    Ok(())
})
```

注意错误类型是 **`Box<dyn std::error::Error>`**,不是我们 B2 定义的 `AppResult` 里的 `AppError`。为什么?

### 历史/兼容性原因

Tauri 这套 API 在你的项目存在之前就定型了。它不可能知道用户会自定义一个叫 `AppError` 的类型,所以选了 Rust 生态最通用的"任何错误"占位符 —— `Box<dyn std::error::Error>`(中文常说"任意错误对象")。

### `Box<dyn std::error::Error>` 是什么?

- `dyn std::error::Error` = **任何**实现了 `std::error::Error` trait 的类型 (trait object)
- 但 trait object 大小不固定(可能是任何东西),不能直接放栈上
- 所以包一层 `Box<...>` 把它放堆上,栈上只放指针

效果:这个返回类型能接 **任何** Rust 错误,你不需要先定义"我的所有错误类型"。

### 我们 `AppError` 怎么塞进去?

只要 `AppError` 实现了 `std::error::Error` trait —— 而我们用 `#[derive(thiserror::Error)]`,**就已经实现了**。所以可以这样:

```rust
.setup(|app| {
    init_state(app)?;          // init_state 返回 AppResult<()>,? 自动转
    Ok(())
})
```

`?` 在这里做了两件事:

1. 如果 `init_state` 返回 `Err(AppError)`,`?` 跳出 closure 返回 `Err(...)`
2. 因为返回类型是 `Box<dyn Error>`,`AppError` 满足 `: Error`,**编译器自动把 `AppError` 装箱**(隐式 `From<AppError> for Box<dyn Error>`,标准库提供)

这是 B2 学的 `?` 跨类型转换的实际应用 —— 不仅能转你自己的错误,**也能转任意 `: Error` 类型进 `Box<dyn Error>`**。

### 为什么 Tauri 不直接收 `AppResult`?

如果 Tauri 强求你用某个错误类型,所有项目都要把自己的错误转成 Tauri 的;**任意错误对象** 完全反过来 —— 各项目用各项目的错误,在边界处装箱即可。这是库 API 设计的常见取舍。

---

## 5. `block_on` 在 sync setup 里调 async `AppState::new`(本节最大坑)

### 矛盾

- `AppState::new` 是 `async fn`(B4 写好了)→ 调它需要 `.await`
- `.setup(|app| { ... })` closure 是 **sync** 的 → 不能 `.await`

两者打架。怎么办?

### `tauri::async_runtime::block_on` 救场

```rust
.setup(|app| {
    let handle = app.handle().clone();
    let state = tauri::async_runtime::block_on(async move {
        AppState::new(&handle).await
    })?;
    app.manage(state);
    Ok(())
})
```

`block_on(future)` 的意思:"**阻塞当前线程**,跑这个 async future 到完成,把结果返回给 sync 代码"。这是 sync ↔ async 的标准桥接方式。

### 为什么是 `tauri::async_runtime::block_on` 而不是 `tokio::runtime::Handle::current().block_on`?

Tauri 有自己一层 `async_runtime` 抽象:

- 默认 `async_runtime` 后端就是 tokio
- 但 Tauri 想让自己的 API 对后端不感(理论上能换),所以包了一层
- 直接用 `tokio::...` 也能跑,**但官方推荐用 Tauri 包装版**,避免假设 runtime 是 tokio

实际效果一样,选 Tauri 的版本图个语义干净。

### `block_on` 的代价?

- **阻塞调用线程**直到 future 完成。setup 阶段就一次,app 还没真正起来,阻塞几百毫秒没人感知 —— 无所谓
- 如果你在某个 hot path 的 sync 函数里反复 `block_on`,可能死锁(详见 tokio runtime panics)。B5 我们只在 setup 里调一次,完全安全

### 有没有更优雅的做法?

有 —— **`async setup`**。Tauri 2 后续版本可以这样:

```rust
.setup(|app| async move {
    let handle = app.handle().clone();
    let state = AppState::new(&handle).await?;
    app.manage(state);
    Ok(())
})
```

但这要求 Tauri 版本支持 async setup,API 还不算完全稳定。**B5 我们走稳妥路线 `block_on`**,如果你查文档发现你装的 Tauri 版本支持 async setup 也可以用,review 时再讨论。

### 关于 `app.handle().clone()`

`app: &mut App` 是 setup closure 的参数,生命周期短(只在 closure 内有效)。`AppState::new` 收 `&AppHandle` —— `AppHandle` 是 Tauri 提供的、可以**克隆**且**'static** 的 handle 类型,允许跨 closure / 跨 thread 传递。

实际写起来:

```rust
.setup(|app| {
    let handle = app.handle().clone();              // 拿一份可跨边界的 handle
    let state = tauri::async_runtime::block_on(async move {
        AppState::new(&handle).await
    })?;
    app.manage(state);                               // app 还在 closure 里,直接用
    Ok(())
})
```

`app.handle()` 返回 `&AppHandle`,`.clone()` 是 cheap clone(内部 Arc)。

---

## 6. `app.manage(state)`:TypeMap 是怎么按类型查找的

### 现象

```rust
app.manage(state);          // 存
```

然后在任何命令里:

```rust
#[tauri::command]
async fn whatever(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let path = state.workspace.read().await.clone();
    ...
}
```

**没有按名字注册**,Tauri 自动按 `AppState` 类型把对应实例传进来。怎么做到的?

### 内部:`TypeId` 索引的 `HashMap`

`app.manage::<T>(t: T)` 干两件事:

1. 拿 `TypeId::of::<T>()` —— Rust 的"类型指纹",编译期决定的一个数字
2. 把 `(TypeId, Box<dyn Any>)` 存进内部的 `HashMap<TypeId, Box<dyn Any>>`(Any 是"任意类型"的 trait object)

命令收到 `tauri::State<'_, T>` 参数时:

1. 用 `TypeId::of::<T>()` 查 HashMap
2. 拿到 `&dyn Any`,downcast 回 `&T`
3. 包成 `tauri::State<'_, T>` 交给命令

### 关键约束

**每种类型只能 manage 一次**。如果你 `app.manage(state1); app.manage(state2);` 都是 `AppState`,后者覆盖前者。这是按"类型"查找的代价 —— 同一类型多份没法区分。

我们项目里 `AppState` 就是单例,完美匹配这套机制。

### 命令函数怎么"知道"要从 manage 拿?

`tauri::State<'_, T>` 是个**特殊参数类型**,`#[tauri::command]` 宏看到这种类型就自动注入 manage 里的对应实例。其它特殊参数还有 `AppHandle` / `Window` / `Channel<T>` 等(后续阶段会遇到)。

---

## 7. `generate_handler!` 宏:编译期生成 IPC 路由表

### 现状

```rust
.invoke_handler(tauri::generate_handler![greet])
```

这就一个 demo 命令。B5 要扩成全部 B3 注册的 todo 命令:

```rust
.invoke_handler(tauri::generate_handler![
    commands::workspace::get_workspace,
    commands::workspace::select_workspace,
    commands::workspace::set_workspace,
    commands::fs::fs_read,
    commands::fs::fs_write,
    // ... 一堆
])
```

### 宏展开做了什么

简化版思路:

```rust
// 你写的
generate_handler![commands::workspace::get_workspace, commands::fs::fs_read]

// 宏展开后(概念上)
move |invoke: tauri::Invoke| {
    match invoke.message.command() {
        "get_workspace" => {
            // 解析参数 → 调 get_workspace → 序列化返回值
        }
        "fs_read" => {
            // 同上
        }
        _ => { /* 404 */ }
    }
}
```

**好处**:整个 IPC 路由表在**编译期**就 wire 好,运行时只是查表。命令名拼错、参数类型不对、忘记 `pub`,编译就报错。

### 想看真容

```bash
cargo install cargo-expand        # 一次性装
cargo expand --bin yukin --color always | less       # 看你的 lib.rs 宏展开后什么样
```

`cargo expand` 把所有宏展开成最终 Rust 代码。第一次看会觉得"哇这么多",但能彻底打消"宏是黑魔法"的疑虑。

### 实际怎么列这些命令

B3 你已经建好骨架,每个文件里有若干 `pub async fn xxx(...) -> AppResult<...>`。**只要这些函数标了 `#[tauri::command]` 且是 `pub`**,就能填进 `generate_handler!`。

[phase-b-rust-foundation](../2026-06-06-phase-b-rust-foundation.md) §6 列了完整命令清单。但我们 B3 还没把每个命令都写 `#[tauri::command]` 宏(可能写了几个,可能都没写),B5 之前你**先打开每个 commands/*.rs,把每个 `pub async fn` 加上 `#[tauri::command]`**(如果还没的话)—— 这一步顺手在 B5 做掉。

---

## 8. `tracing` vs `println!` / `log`:结构化日志

### `println!` 的问题

- 全打到 stdout,没有级别(info / warn / error)
- 没有结构(只有字符串)
- 不能按模块过滤
- 生产环境想关掉只能注释掉

### `log` crate(老一代)

- 有级别(`log::info!` / `warn!` / `error!`)
- 但是结构化弱:消息还是字符串
- 需要单独装 logger 后端

### `tracing` crate(现代选择,我们用这个)

- 有级别
- **结构化**:每条日志可以带 K-V 字段,被后端解析成 JSON
- **span** 概念:跟踪一个 async 任务从启动到结束的整个生命周期
- 多种后端可插(终端彩色 / JSON 文件 / OpenTelemetry 上云)

例子:

```rust
use tracing::{info, error, instrument};

#[instrument(skip(state))]       // 自动给函数加 span,排除大字段
async fn fs_read(path: String, state: State<'_, AppState>) -> AppResult<String> {
    info!(path = %path, "reading file");
    let content = tokio::fs::read_to_string(&path).await
        .inspect_err(|e| error!(error = %e, "read failed"))?;
    Ok(content)
}
```

打出来(终端):

```
2026-06-08T12:34:56  INFO fs_read{path="package.json"}: yukin::commands::fs: reading file
```

JS 端没有完美对应物,最接近 `pino` / `winston` 这种 structured logger,但 tracing 的 span 概念更强。

### tracing 的两层

- `tracing` —— 提供 `info!` / `warn!` / `error!` / `debug!` / `trace!` 宏,以及 `Span` 概念
- `tracing-subscriber` —— **后端**,决定日志怎么输出(终端 / 文件 / JSON / ...)

你的代码里只用 `tracing`,**初始化时**用 `tracing-subscriber` 装一个后端。这是 Rust 生态典型模式 —— "trait + impl 分包"(facade pattern)。

### B5 的初始化(最小版)

```rust
use tracing_subscriber::{fmt, EnvFilter};

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,yukin=debug")))
        .init();
}
```

意思:

- `fmt()` —— 用"格式化输出到终端"后端(彩色、有时间戳)
- `.with_env_filter(...)` —— 加级别过滤
- `EnvFilter::try_from_default_env()` —— 读 `RUST_LOG` 环境变量,失败就 fallback
- `.init()` —— 安装为全局后端(全进程一次)

---

## 9. `RUST_LOG` 如何生效

启动时写 `RUST_LOG=yukin=debug pnpm tauri dev`:

- `yukin=debug` 意思:`yukin` 这个 crate 及其子模块,显示 debug 及以上(debug < info < warn < error)
- 多个模块可以逗号分隔:`RUST_LOG=yukin=debug,reqwest=info,sqlx=warn`
- 不写 `RUST_LOG`,我们 fallback 是 `info,yukin=debug` —— 全局 info,yukin 自家更详细

### crate 名是什么?

看 `Cargo.toml` 的 `[package].name`:

```toml
[package]
name = "yukin"
```

所以 `yukin` 就是 crate 名。所有模块路径都是 `yukin::commands::fs` 这种形式。

> 注:`src-tauri/Cargo.toml` 里你看到的 name 可能是 `yukin` 或 `yukin_lib`,以那个为准。模块过滤用真实的 crate 名。

### 验证 EnvFilter 生效

启动后看终端,有 `INFO`/`DEBUG` 等彩色日志输出 = 后端装好了。手动测一下:在 `setup` 内随便 `info!("setup done")`,看到这行 = `RUST_LOG` 没拦截。

---

## 10. 你的任务

### 改 `src-tauri/src/lib.rs`

按以下结构改:

```rust
mod agent;
mod commands;
mod error;
mod llm;
mod path_safety;
mod state;
mod tools;

pub use error::{AppError, AppResult};
pub use state::AppState;

// ↓ 删 greet,如果还在的话

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,yukin=debug")),
        )
        .try_init();        // try_init 不 panic on second call,测试更友好
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(async move {
                AppState::new(&handle).await
            })?;
            app.manage(state);
            tracing::info!("yukin setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ↓ 填全 commands/*.rs 里所有 #[tauri::command] 函数
            // 写法:模块路径::函数名
            commands::workspace::get_workspace,
            commands::workspace::select_workspace,
            commands::workspace::set_workspace,
            commands::fs::fs_read,
            commands::fs::fs_write,
            commands::fs::fs_edit,
            commands::fs::fs_list_dir,
            commands::fs::fs_glob,
            commands::fs::fs_exists,
            commands::keychain::key_set,
            commands::keychain::key_exists,
            commands::keychain::key_delete,
            commands::keychain::key_list_providers,
            commands::memory::memory_save,
            commands::memory::memory_recall,
            commands::memory::memory_list,
            commands::memory::memory_delete,
            commands::session::session_create,
            commands::session::session_list,
            commands::session::session_update,
            commands::session::session_delete,
            commands::session::session_append_message,
            commands::session::session_load_messages,
            commands::agent::chat_send,
            commands::agent::chat_abort,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 重点提醒

- **`AppState::new` 现在还是 `unimplemented!()`** —— 跑到 setup 会 panic。这是预期的:B5 跑通到 *启动 panic* 已经完成主要目标。Phase C 才会真正实现 `AppState::new`,届时 setup 不再 panic、窗口能起来。
- **如果你不想现在 panic**,可以**临时**在 `AppState::new` 里改成 `Ok(AppState { workspace: ..., db: None, http: reqwest::Client::new(), runs: ... })` 一个最简返回。这样 B5 就能验证窗口启动。后续 Phase C 再加 sqlite/migration。**这个选择记到主进度文档"实际收获"小节**。
- **`generate_handler!` 里列哪些命令**:看你 B3 实际写了哪些 `#[tauri::command]`。**没写 `#[tauri::command]` 的 `pub async fn` 不能填**,会编译报错"找不到这个命令"。
- **命令路径必须完整**:`commands::fs::fs_read` 而不是只写 `fs_read`(后者要在 `lib.rs` 顶部 `use` 进来,可读性反而差,主流 Tauri 项目都写完整路径)。

---

## 11. 验证

```bash
cd src-tauri
cargo check         # 必须通过
```

然后:

```bash
# 在项目根目录
pnpm tauri dev
```

预期行为(两种):

### 选项 A:`AppState::new` 仍是 `unimplemented!()`

启动后会 panic,日志看到 `not yet implemented`。**这也是 B5 完成的证据**,因为 panic 发生 = 你的 setup 跑到了 `AppState::new` 调用,前面所有(tracing 初始化 / plugin 注册 / 进入 setup closure)都成功了。可以打钩。

### 选项 B:`AppState::new` 临时返回最简 Ok

```rust
impl AppState {
    pub async fn new(_app: &AppHandle) -> AppResult<Self> {
        Ok(Self {
            workspace: RwLock::new(None),
            db: None,
            http: reqwest::Client::new(),
            runs: RwLock::new(HashMap::new()),
        })
    }
}
```

启动后窗口起来,前端 React 显示。在 devtools console 跑:

```js
await __TAURI__.core.invoke('get_workspace')
```

应该返回 `{ code: "other", message: "todo" }`(因为 B3 命令体是 `Err(AppError::Other("todo".into()))`)。

**强烈推荐选项 B**,因为:

- 验证更彻底(window 真起来 = 整条链路打通)
- B6 改 capability/CSP 也需要窗口能起来才能验
- Phase C 实现 db 时只改 `AppState::new`,不影响 B5 的其它工作

### 验证 tracing 输出

启动时终端应该看到类似:

```
2026-06-08T12:34:56  INFO yukin: yukin setup complete
```

如果没看到 `yukin setup complete`,说明 tracing 没初始化好 / 或 setup hook 早 panic 没跑到这行。

再试:

```bash
RUST_LOG=yukin=debug pnpm tauri dev
```

应该看到更多 debug 级别日志(虽然我们 B5 暂时只放了一条 info)。

---

## 12. 卡点 / 易错点提醒

### 编译错

- **`AppState::new` 签名不匹配** —— B4 写的是 `pub async fn new(_app: &AppHandle) -> AppResult<Self>`,setup 里要按这个签名调
- **`#[tauri::command]` 缺失** —— `generate_handler!` 里列的函数必须都标了这个宏,否则编译报"找不到命令"
- **plugin 名字写错** —— `tauri_plugin_dialog` 不是 `tauri_plugin_dialogs`,留意 Cargo.toml 里的下划线
- **`pub use state::AppState`** —— B4 已经加了的话别重复加

### 运行错

- **`AppState::new` 是 `unimplemented!()`** —— 选项 A 预期,选项 B 改成最简 Ok
- **`app.handle().clone()` vs `app.handle()`** —— `&AppHandle` 不能 move 进 async closure,必须 `.clone()` 先拿到 owned handle
- **窗口起不来,日志没输出** —— 可能 `init_tracing()` 没调,或者 setup hook 立即 panic 前没机会 flush stdout

### 设计陷阱

- **plugin 注册顺序** —— 99% 无所谓,但保持 `opener → dialog → sql` 这种字母序 / 重要性序,review 友好
- **`generate_handler!` 漏列命令** —— 漏列的命令前端 invoke 不到,运行时报"command not found"。**对照 B3 文件清单一个一个加,别凭记忆**
- **`block_on` 在 setup 之外用** —— B5 只在 setup 用一次,别在命令里嵌 `block_on`(命令本来就 async,直接 await 即可)

---

## 13. 写完贴给我 review 时,我会重点看

- `init_tracing` 用了 `try_init` 还是 `init`?(`try_init` 测试更友好)
- EnvFilter fallback 字符串合理吗?(默认 info,yukin 自家 debug)
- `setup` closure 里 `app.handle().clone()` 有没有忘掉 `.clone()`
- `block_on(async move { AppState::new(&handle).await })?` 这一行结构对不对
- `app.manage(state)` 在 `block_on` **之后**(顺序错了 state 还没生成就 manage)
- 选项 A 还是 B?如果是 B,临时实现的 `AppState::new` 字段顺序/类型对不对
- `generate_handler!` 列表跟 B3 commands/ 下所有 `#[tauri::command]` 一一对应吗
- plugin 注册行有没有少 `dialog` 或 `sql`
- 启动后:`pnpm tauri dev` 是 panic(A)还是窗口起来(B)?devtools 测一条 invoke 看返回
- `RUST_LOG=yukin=debug pnpm tauri dev` 有更详细日志吗
- 你的"选项 A/B"决定有没有记到主进度文档

---

## 14. 进阶(可选,做不做都行)

如果当天有余力,这两个进阶值得做:

### 14.1 用 `cargo expand` 看 `generate_handler!` 展开

```bash
cargo install cargo-expand
cd src-tauri
cargo expand 2>&1 | grep -A 50 'invoke_handler'
```

会看到一个巨大的 match arm,印证第 7 节的概念。

### 14.2 给一个命令加 `#[tracing::instrument]` 试效果

挑 `get_workspace`(最简单的命令),改成:

```rust
#[tracing::instrument(skip(_state))]
#[tauri::command]
pub async fn get_workspace(
    _state: tauri::State<'_, AppState>,
) -> AppResult<Option<String>> {
    tracing::info!("called");
    Err(AppError::Other("todo".into()))
}
```

启动后前端 invoke,终端能看到:

```
INFO get_workspace: yukin::commands::workspace: called
```

`get_workspace:` 这部分就是 span。Phase F+ 真正写 LLM stream 时这种 span tracking 价值会爆炸式上升。
