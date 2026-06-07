# B2 — `error.rs`:`AppError` + `?` + `From` trait(概念课)

> 创建日期: 2026-06-07
> 配套: [phase B 学习总入口](../2026-06-07-phase-b-learning.md) / [phase B 架构定义](../2026-06-06-phase-b-rust-foundation.md)
> 用途: B2 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

这一步是学习重头戏 1。分 5 节讲:

1. Rust 错误处理的整体哲学
2. `?` 运算符的脱糖
3. `From` trait —— `?` 跨类型工作的引擎
4. `thiserror` 的 `#[derive(Error)]` + `#[from]` 帮你省了什么手写
5. 为什么要手写 `impl Serialize`(Tauri ↔ 前端约束)

然后是 `AppResult<T>` type alias、任务、自检、卡点提醒。

---

## 1. Rust 错误处理的整体哲学

Rust 没有 `try/catch/throw`。所有可能失败的函数,**必须在签名里说出来**:

```rust
fn read_file(path: &str) -> Result<String, std::io::Error>
//                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//                          签名告诉调用方:这个函数可能失败
```

`Result<T, E>` 是个标准库 enum:

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

调用方拿到 `Result` 后**不能假装它是 T**,必须 match / unwrap / `?` / `if let` 才能拿到里面的值。这就是 Rust 的核心承诺:**错误在类型系统里可见,编译器逼你处理**。

代价?如果一个函数链里 5 步都可能错,朴素写法长这样:

```rust
fn pipeline() -> Result<Data, MyError> {
    let a = match step1() {
        Ok(v) => v,
        Err(e) => return Err(MyError::from(e)),
    };
    let b = match step2(a) {
        Ok(v) => v,
        Err(e) => return Err(MyError::from(e)),
    };
    // ... 三十行 match
}
```

读起来想哭。`?` 运算符就是解决这个的。

---

## 2. `?` 运算符的脱糖

把上面那坨简化成:

```rust
fn pipeline() -> Result<Data, MyError> {
    let a = step1()?;
    let b = step2(a)?;
    let c = step3(b)?;
    Ok(c)
}
```

**`x?` 等价于以下 match 表达式**:

```rust
match x {
    Ok(v)  => v,                              // 是 Ok 就拆出来当本行的值
    Err(e) => return Err(From::from(e)),      // 是 Err 就立刻 return,顺手把 e 转成函数返回的错误类型
}
```

注意第二行的 `From::from(e)` —— 这是 `?` 的**真正魔法**。这一调用让 `?` 能把 `step1` 返回的 `io::Error` 自动转成你函数声明要返回的 `MyError`,**前提是你为这两个类型实现了 `From<io::Error> for MyError`**。

如果两个错误类型完全相同(`?` 用在 `Result<_, io::Error>` 里的 `Result<_, io::Error>`),`From::from` 退化成 identity,无开销。

---

## 3. `From` trait

`From` 是标准库 trait:

```rust
trait From<T> {
    fn from(value: T) -> Self;
}
```

"我能从 T 造出 Self"。手写一个例子:

```rust
struct MyError(String);

impl From<std::io::Error> for MyError {
    fn from(e: std::io::Error) -> Self {
        MyError(format!("io error: {}", e))
    }
}
```

写了这个 impl 之后,以下三种写法都能工作:

```rust
let s = MyError::from(some_io_error);          // 显式调
let s: MyError = some_io_error.into();         // 隐式调,因为 impl From<X> for Y 自动给了 impl Into<Y> for X
let s = function_returning_my_error(some_io_error?);  // ? 调,因为 ? 内部用了 From::from
```

**关键认识**:`?` 不是关键字魔法 —— 它就是 match + `From::from` 的语法糖。你为每个想"自动转换"的错误源类型写一个 `From` impl,`?` 就能在那条边界自动跨。

如果你的 `MyError` 想 cover 10 种错误源(io、http、db、json、...),你就要手写 10 个 `From` impl。这就是 `thiserror` 进场的地方。

---

## 4. `thiserror` 的 `#[derive(Error)]` + `#[from]`

`thiserror` 是个 proc-macro crate,核心两个东西:

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("io: {0}")]                      // 控制 Display 输出
    Io(#[from] std::io::Error),              // #[from] 自动生成 impl From<io::Error> for AppError

    #[error("workspace not set")]            // 无字段变体
    NoWorkspace,

    #[error("path escapes workspace: {0}")]  // {0} 引用第 0 个字段
    PathEscape(String),
}
```

宏展开后等价于你手写以下三大块(粗略示意):

```rust
// 1. Debug 是 #[derive(Debug)] 给的,不算 thiserror 功劳

// 2. Display(用于 println!("{}", err) 和日志)
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "io: {}", e),
            AppError::NoWorkspace => write!(f, "workspace not set"),
            AppError::PathEscape(s) => write!(f, "path escapes workspace: {}", s),
        }
    }
}

// 3. std::error::Error(让你的 enum 成为合格 Rust error)
impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(e) => Some(e),  // 链式错误,可以一路 .source() 上溯
            _ => None,
        }
    }
}

// 4. #[from] 标记字段对应的 From impl
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}
```

省了多少手写代码自己看。**记住的关键**:

- `#[error("...")]` 控 Display 输出格式,`{0}` `{1}` 引用元组字段,`{name}` 引用具名字段
- `#[from]` 只能用在**单字段**变体上,自动给你生成 `From<T> for Outer`
- 一个 `From` 源类型不能在同一个 enum 里被标 `#[from]` 两次(逻辑上歧义,编译报错)

这跟 B1 提到的 `anyhow` vs `thiserror` 对应起来:**库代码用 thiserror 写精确枚举,app 代码用 anyhow 当黑盒**。我们的 `AppError` 是给 Tauri 命令返回的,前端要 match `code` 显示不同 UI,所以必须枚举 —— 用 thiserror。

---

## 5. 为什么要手写 `impl Serialize`(Tauri 关键约束)

Tauri 命令的签名长这样:

```rust
#[tauri::command]
async fn get_workspace() -> Result<Option<String>, AppError> {
    // ...
}
```

当返回 `Err(AppError::PathEscape("../etc/passwd".into()))` 时,Tauri 会**把这个 Err 用 serde 序列化成 JSON,发给前端**,前端拿到一个 JS 对象 reject 出去给 invoke 的 caller。

如果你 `#[derive(serde::Serialize)]` 给 `AppError`,默认行为是按 serde 的 enum representation 规则序列化,长这样:

```json
{ "Io": { "kind": "NotFound", "message": "..." } }
```

变体名当 key,字段当 value。问题:

- 前端没法稳定 match —— 同一种"路径错"出现两种 key(`PathEscape` 一种,`Io::NotFound` 另一种)
- `io::Error` 自己没实现 Serialize,默认 derive 会**编译失败**

所以 doc 让你**手写** `impl Serialize`,统一拍平成:

```json
{ "code": "path_escape", "message": "path escapes workspace: ../etc/passwd" }
```

`code` 是你定的 stable 短码(适合前端 switch),`message` 用 `Display`(就是 `format!("{}", self)` 的输出,正好 thiserror 给你了)。

手写 impl Serialize 的最小形态:

```rust
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let code = match self {
            AppError::NoWorkspace    => "no_workspace",
            AppError::PathEscape(_)  => "path_escape",
            AppError::Io(_)          => "io",
            AppError::Db(_)          => "db",
            AppError::Keyring(_)     => "keyring",
            AppError::DialogCancelled => "dialog_cancelled",
            AppError::Shell(_)       => "shell",
            AppError::Http(_)        => "http",
            AppError::Llm(_)         => "llm",
            AppError::Cancelled      => "cancelled",
            AppError::Other(_)       => "other",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", code)?;
        s.serialize_field("message", &self.to_string())?;  // 用 Display
        s.end()
    }
}
```

读懂这段:`serializer.serialize_struct("AppError", 2)?` 开始一个有 2 个字段的"struct"(在 JSON 里就是对象),`serialize_field` 写字段,`end()` 收尾。`self.to_string()` 调的就是 thiserror 给你 derive 的 `Display::fmt`,所以你不用再写 message 格式。

---

## 6. `AppResult<T>` type alias

最后这个小语法糖:

```rust
pub type AppResult<T> = std::result::Result<T, AppError>;
```

`type` 关键字是类型别名(不是新类型,跟 TS 的 `type` 一模一样)。写了这个之后:

```rust
// 之前
fn foo() -> Result<String, AppError> { ... }
// 之后
fn foo() -> AppResult<String> { ... }
```

整个 codebase 短一截,统一。这是 Rust 圈约定俗成的 pattern,几乎每个 app 都有自己的 `Result` 别名(`anyhow::Result<T>` 也是这种东西,它内部 = `Result<T, anyhow::Error>`)。

---

## 7. 你的任务

新建 `src-tauri/src/error.rs`,完成 4 件事:

1. **定义 `AppError` enum**,按 phase-b doc 第 2 节列的 11 个变体。`#[derive(thiserror::Error, Debug)]`,变体上加 `#[error("...")]`,IO / Db / Keyring / Http 变体用 `#[from]` 标记字段。
2. **手写 `impl serde::Serialize for AppError`**,产出 `{ code: "...", message: "..." }`。code 你自己起短码(snake_case),message 用 `self.to_string()`。
3. **定义 `pub type AppResult<T> = ...`**。
4. **在 `src-tauri/src/lib.rs` 顶部加 `mod error;`**(告诉 crate 这个模块存在),否则 cargo 看不到这个文件。可以同时 `pub use error::{AppError, AppResult};` 让外部直接 `use crate::AppError`。

写一个临时函数自验 `?` 工作:

```rust
// 临时放在 lib.rs 或 error.rs 末尾,验证完删掉
fn _test_question_mark() -> AppResult<String> {
    let content = std::fs::read_to_string("/this/path/does/not/exist")?;
    //                                                                ^ 这里 io::Error 自动 → AppError
    Ok(content)
}
```

如果 `?` 那行编译过了,说明 `#[from]` 工作了 —— 不需要真跑这个函数,编译过就证明类型转换链通了。

---

## 8. 验证步骤

```bash
cd src-tauri
cargo check
```

应该通过,可能会有一条 `_test_question_mark` 未使用的 dead_code warning,不管它(完了你会删)。

---

## 9. 卡点 / 易错点提醒

- **`#[from]` 不能标元组里的多个字段** —— 必须是单字段变体,例如 `Io(#[from] std::io::Error)`,不是 `Io(#[from] std::io::Error, String)`
- **同一个源类型只能 `#[from]` 一次** —— 比如不能 `Db(#[from] sqlx::Error)` + `OtherDb(#[from] sqlx::Error)`,编译器报歧义
- **`impl Serialize` 的 trait bound** —— `where S: serde::Serializer` 别忘了,`?` 末尾的 `s.end()?` 也别漏
- 在 `lib.rs` 加 `mod error;` 前,`use crate::error::AppError` 会找不到 —— "mod 声明 = 把这个文件挂进模块树"

---

## 10. 写完贴给我 review 时,我会重点看

- `#[from]` 用对没
- code 短码起得合不合理(snake_case、前端 switch 友好)
- `impl Serialize` 套路对不对(`serialize_struct` 字段数、`self.to_string()`、`s.end()?`)
- `AppResult` type alias 位置
- `_test_question_mark` 是否真的编译过(证明 `?` 跨类型链路通了)

任何编译报错也贴出来,Rust 的编译错误信息很啰嗦但读懂之后能学到东西。
