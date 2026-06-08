# B4 — `state.rs`:`AppState` + `Arc<RwLock>` + `Send + Sync`(概念课)

> 创建日期: 2026-06-08
> 配套: [phase B 学习总入口](../2026-06-07-phase-b-learning.md) / [phase B 架构定义](../2026-06-06-phase-b-rust-foundation.md)
> 用途: B4 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

这是学习重头戏 2 —— **Rust 并发模型**。分 6 节:

1. Rust 并发的整体哲学(对比 JS 单线程事件循环)
2. `Send` / `Sync` 两个标记 trait —— 编译期决定"能不能跨线程"
3. `'static` 生命周期约束 —— 长寿命数据
4. `tokio::sync::RwLock` vs `std::sync::RwLock`(本节最大的坑)
5. `Arc<T>` 共享所有权 —— 多任务持有同一份数据
6. `RwLock<Option<T>>` 和 `RwLock<HashMap<K, V>>` 两个常见 pattern

然后是 `db` 字段决策、任务、自检、卡点提醒。

---

## 1. Rust 并发哲学(对比 JS)

### JS 的世界

JS 是**单线程事件循环**:所有 JS 代码运行在一个线程上,`await` 只是把当前任务挂起、让出事件循环。没有真正的并行执行 JS 代码,所以"两个任务同时改同一个 object"不可能发生 —— 你不需要锁。

```js
let counter = 0;
async function inc() {
  counter += 1;        // 永远安全,JS 不会在 += 中间切到别的任务
}
```

### Rust 的世界

Rust 默认假设**多线程并行**。`tokio` runtime 默认开 N 个 worker 线程(N = CPU 核数),async 任务在这些线程间被调度执行。**两个任务真的可能在两个 CPU 核同时跑**。

```rust
// 这段代码在 Rust 是 race condition,编译器直接拒绝
let mut counter = 0;
tokio::spawn(async move { counter += 1; });    // task 1
tokio::spawn(async move { counter += 1; });    // task 2  ← 编译报错: counter 已被 move
```

编译器逼你想清楚:**这个数据要不要被多线程共享?如果要,你怎么保证安全?** 这就是 `Send` / `Sync` / `Arc` / `Mutex` / `RwLock` 一整套工具的存在理由。

### Tauri 命令的并发现实

Tauri 用 tokio runtime,前端可以**同时**发起多个 invoke,每个命令在不同 task / 不同线程上跑。所以:

```rust
// 多个前端调用并发执行,都要拿到 AppState 的引用
#[tauri::command]
async fn get_workspace(state: tauri::State<'_, AppState>) -> AppResult<Option<String>> { ... }

#[tauri::command]
async fn set_workspace(state: tauri::State<'_, AppState>, path: String) -> AppResult<()> { ... }
```

如果 `AppState` 内部有可变字段(workspace 路径会被用户改),就要锁。

---

## 2. `Send` / `Sync` 两个标记 trait

### 定义

| trait | 含义 | 反例 |
|-------|------|------|
| `Send` | 此类型可以**转移**所有权到另一个线程 | `Rc<T>` 不是 Send(引用计数不是原子的,两个线程同时 clone 会乱) |
| `Sync` | 此类型可以**多线程同时共享 `&T`** | `Cell<T>` 不是 Sync(允许内部可变但没加锁) |

记忆口诀:**Send 是搬过去,Sync 是借给多个看**。

### 关键性质

- **自动实现(auto trait)**:你的 struct 不需要手写 `impl Send`,编译器根据字段自动推导 —— 所有字段都 `Send`,struct 就 `Send`。`Sync` 同理。
- **大多数普通类型两者都满足**:`String` / `Vec<T>` / `HashMap<K,V>`(T/K/V 满足时) / `PathBuf` / `i32` ...
- **少数类型不满足**:`Rc<T>`(用 `Arc<T>` 替代)、`RefCell<T>`(用 `Mutex<T>` / `RwLock<T>` 替代)、裸指针 `*const T` / `*mut T`

### Tauri 为什么要 `T: Send + Sync + 'static`?

Tauri 把你 manage 的 state 存进 `TypeMap` 里(按类型查找的全局 map),**任何命令任何线程**随时按类型取一份 `&State<T>`:

- 不同线程拿到 `&State<T>` → 多线程共享 `&T` → 需要 `Sync`
- state 可能被 send 到其它任务里 → 需要 `Send`
- state 活整个进程,不能借短命数据 → 需要 `'static`

如果 `AppState` 任何一个字段不满足这三个,`app.manage(state)` 编译报错。这就是为什么 B4 的"验证"会说:能 `manage` 不报错 = 三个约束都满足。

### 实操影响

写 `AppState` 时,**每个字段类型都要满足 Send + Sync**。我们用的:

- `RwLock<Option<PathBuf>>` — `tokio::sync::RwLock<T>` 在 `T: Send` 时是 `Send + Sync` ✓
- `Option<sqlx::SqlitePool>` — `SqlitePool` 内部是 `Arc<Pool>`,Send+Sync ✓
- `reqwest::Client` — 内部 `Arc`,Send+Sync ✓
- `RwLock<HashMap<String, CancellationToken>>` — 同上 ✓

全部 OK,不需要我们手动加任何东西。

---

## 3. `'static` 生命周期

简单理解:**这个值的所有数据要么自己拥有,要么是程序级常量,不依赖任何短命借用**。

```rust
// 'static 的例子
let s: String = String::from("hi");        // String 自己拥有 heap 数据,满足 'static
let n: i32 = 42;                            // 值类型,'static
let lit: &'static str = "hello";            // 字符串字面量本身就是 'static
```

```rust
// 不是 'static 的例子
fn bad<'a>(x: &'a str) -> impl Send + 'a { x }    // 返回值借用了 x,只活 'a,不是 'static
```

Tauri state 要 `'static`,因为 state 活整个进程,不能借用任何函数局部数据。我们的 `AppState` 所有字段都是"拥有型"(`String` / `PathBuf` / `RwLock` 包的拥有型),自动 `'static`。

---

## 4. `tokio::sync::RwLock` vs `std::sync::RwLock`(**本节最大坑**)

### 两个 RwLock

Rust 有两个 `RwLock`,长得几乎一样,选错代价巨大:

| | `std::sync::RwLock` | `tokio::sync::RwLock` |
|---|---|---|
| 来源 | 标准库 | tokio crate |
| 阻塞方式 | **OS 线程阻塞** | 任务挂起(yield 给 runtime) |
| 跨 `.await` 持锁 | **死锁/性能崩盘** | 安全 |
| 适用场景 | 短临界区、不 await(sync 代码) | async 代码 |
| API | `.read().unwrap()` / `.write().unwrap()`(可能 PoisonError) | `.read().await` / `.write().await`(没有 PoisonError) |

### 为什么 std RwLock 跨 await 会死锁?

设想一个场景,用 `std::sync::RwLock`:

```rust
async fn bad() {
    let guard = state.workspace.write().unwrap();   // 拿写锁
    let data = fetch_from_network().await;          // ← 这个 await 期间锁还在!
    *guard = data;
}
```

`fetch_from_network().await` 期间,**当前 task 被挂起**,但 std RwLock 不知道这件事 —— 它是 OS 级锁,只认线程。

现在:

1. Task A 在线程 1 上拿了写锁,然后 await,挂起 → 线程 1 空闲,被分配去跑 Task B
2. Task B 也想拿这个 RwLock 的读锁 → std RwLock 阻塞整个线程 1 等待
3. Task A 的网络回来了,需要在某个线程上恢复 —— 但线程 1 被 Task B 阻塞,Task A 永远恢复不了
4. → 死锁

`tokio::sync::RwLock` 不会这样,因为它**等待时挂起 task(yield runtime),不是阻塞线程**。同样的代码用 tokio RwLock 就安全。

### 铁律

**async 代码用 `tokio::sync::RwLock`,sync 代码用 `std::sync::RwLock`。**

我们 Tauri 命令全是 async,几乎一定会跨 await(读 db、调网络),**一律用 tokio 的**。

### 题外话:Mutex 同理

`std::sync::Mutex` vs `tokio::sync::Mutex` 是一样的问题。规则一样。

---

## 5. `Arc<T>` 共享所有权

### 问题

Rust 默认**单一所有权**:一个值在任何时刻只有一个所有者。多线程要共享同一份数据,所有权模型就不够用了。

### 解决:`Arc<T>`(Atomically Reference Counted)

`Arc<T>` = 引用计数的智能指针,clone 一份 = 计数 +1。多个线程各持一份 `Arc<T>` clone,谁活着谁就持有,最后一个 drop 时数据才被回收。

```rust
use std::sync::Arc;

let data = Arc::new(String::from("shared"));
let data2 = Arc::clone(&data);           // 计数 +1,data2 也指向同一份数据

tokio::spawn(async move {
    println!("{}", data2);                // task 拥有 data2,可以跨线程
});

println!("{}", data);                     // 主线程还能用 data
```

### 跟 RwLock 组合

`Arc<RwLock<T>>` 是 Rust 异步并发的**经典组合**:

- `Arc` 让多个任务能各自持有一份(共享所有权)
- `RwLock` 让多个任务能安全读写内部数据(内部可变性 + 并发安全)

### 我们 B4 需要 Arc 吗?

**不需要手动包 `Arc`**。原因:Tauri `app.manage(state)` 内部已经做了共享 —— 它把 state 存进 `Arc<_>` 一样的结构,每个命令拿到的 `tauri::State<'_, T>` 是个借用包装,你不用关心。

所以我们直接写:

```rust
pub struct AppState {
    pub workspace: tokio::sync::RwLock<Option<PathBuf>>,    // 注意:RwLock 不需要 Arc 包
    pub runs: tokio::sync::RwLock<HashMap<String, CancellationToken>>,
    ...
}
```

`reqwest::Client` 和 `sqlx::SqlitePool` 也不要再包 `Arc` —— **它们内部自己已经是 Arc**,clone 廉价,本身满足 Send + Sync。

**何时需要手动 `Arc<RwLock<T>>`?** —— 你 spawn task,要把同一份可变数据 clone 给两个 task 各自持有时:

```rust
let shared = Arc::new(RwLock::new(vec![]));
let s1 = Arc::clone(&shared);
tokio::spawn(async move { s1.write().await.push(1); });
let s2 = Arc::clone(&shared);
tokio::spawn(async move { s2.write().await.push(2); });
```

B4 阶段我们写 struct 定义,不 spawn task,不需要 Arc。后面 Phase G(agent loop)会用上。

---

## 6. 两个常见 pattern

### Pattern A: `RwLock<Option<T>>` —— 可选可变全局状态

业务:workspace 路径在启动时**还没设置**,用户后续选了文件夹才设置。

- "可选" → `Option<PathBuf>`
- "可变" → `RwLock<...>`

```rust
pub workspace: RwLock<Option<PathBuf>>,
```

读法(只读):
```rust
let guard = state.workspace.read().await;     // RwLockReadGuard
match &*guard {                                // 解引用 guard 拿到 &Option<PathBuf>
    Some(path) => { /* 用 path */ },
    None => return Err(AppError::NoWorkspace),
}
```

写法:
```rust
let mut guard = state.workspace.write().await;
*guard = Some(new_path);
```

### Pattern B: `RwLock<HashMap<K, V>>` —— 并发 map

业务:agent 跑多个 run 并发,前端能按 `run_id` 点取消。需要一个 map 跟踪每个 run 对应的 `CancellationToken`。

- "可变 map" → `HashMap<String, CancellationToken>`
- "并发安全" → `RwLock<...>`

```rust
pub runs: RwLock<HashMap<String, CancellationToken>>,
```

插入:
```rust
state.runs.write().await.insert(run_id.clone(), token);
```

取出并取消:
```rust
if let Some(token) = state.runs.write().await.remove(&run_id) {
    token.cancel();
}
```

### 为什么 key 是 `String` 而不是 `&str`?

`HashMap` 必须**拥有** key 数据(否则借用过期 map 就崩了),所以 key 类型必须是拥有型 —— `String` 而不是 `&str`。

### 为什么 `runs` 用 `RwLock` 而不是 `Mutex`?

`RwLock` 允许"多读单写",`Mutex` 不分读写都互斥。我们这个 map 读(查 run_id 存在不存在)频率远高于写(新 run 启动 / 取消),用 RwLock 并发性更好。

(其实 `runs` 这种小 map,读写都很短,用 Mutex 也完全 OK。这是个常见的"选哪个"问题,实践中差别可忽略。我们沿用 phase-b doc 的 `RwLock` 即可。)

### `CancellationToken` 怎么用?(B4 不写,先认识)

来自 `tokio_util::sync::CancellationToken`,典型用法:

```rust
// 启动 run 时
let token = CancellationToken::new();
state.runs.write().await.insert(run_id.clone(), token.clone());

let cloned_token = token.clone();
tokio::spawn(async move {
    tokio::select! {
        _ = cloned_token.cancelled() => { /* 收到取消信号 */ }
        result = do_work() => { /* 正常完成 */ }
    }
});

// 取消时
if let Some(t) = state.runs.write().await.remove(&run_id) {
    t.cancel();
}
```

`token` 内部就是个 `Arc`,clone 廉价。同一个 token 可以 clone 多份发给多个 task,任何一份 `.cancel()` 所有副本都感知。

---

## 7. `db: sqlx::SqlitePool` 字段决策

Phase B 不真初始化 db。phase-b doc 列了三个方案,B4 必须选一个:

| 方案 | 优 | 劣 |
|---|---|---|
| **(a) `Option<SqlitePool>`** | 字段就位,Phase C 填值无需改类型/迁移调用方 | 每次用 `db` 要 `.as_ref().ok_or(...)?`,有点噪音 |
| (b) 不加字段,Phase C 再补 | 现在最干净 | Phase C 加字段要改 `AppState::new` 签名 + manage 调用 |
| (c) `OnceCell<SqlitePool>` lazy 初始化 | 语义最准(确实是 lazy) | 多一个 crate 概念,读取代码要 `.get().ok_or(...)?`,跟 (a) 差不多 |

**推荐 (a)**。理由:

- 改动**局部化**:Phase C 只在 `AppState::new` 里把 `None` 改成 `Some(pool)`,struct 定义不动,所有 commands 不动
- 调用噪音可接受:`state.db.as_ref().ok_or(AppError::Other("db not ready".into()))?` 一行,Phase C 起根本不会触发(那时 db 一定是 Some)
- (b) 看似干净,但 struct 定义会变,所有 commands 用 db 的代码可能要随之调整,涟漪比 (a) 大
- (c) 跟 (a) 是同水准,但引入 `once_cell::OnceCell` 这个概念,对初学者多一层认知负担。我们暂时不需要"线程安全 lazy init"语义

**你的决定记到主进度文档"实际收获"小节**,顺便记一句为什么。

---

## 8. 你的任务

新建 `src-tauri/src/state.rs`:

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::AppResult;

pub struct AppState {
    pub workspace: RwLock<Option<PathBuf>>,
    pub db: Option<sqlx::SqlitePool>,                              // Phase C 改 Some(pool)
    pub http: reqwest::Client,
    pub runs: RwLock<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub async fn new(_app: &AppHandle) -> AppResult<Self> {
        unimplemented!()                                            // B5 / Phase C 实现
    }
}
```

然后改 `src-tauri/src/lib.rs`:

```rust
mod agent;
mod commands;
mod error;
mod llm;
mod path_safety;
mod state;          // ← 新增
mod tools;

pub use error::{AppError, AppResult};
pub use state::AppState;        // ← 新增,跟 AppError 一样对外暴露
```

`run()` 函数**先别动**,B5 才会改。

---

## 9. 验证

```bash
cd src-tauri
cargo check
```

应该通过,可能有 unused 警告(`AppState` 没人用),忽略。

**进阶自检(可选,但建议做):** 在 `lib.rs` 末尾加一段:

```rust
// 临时验证 AppState 满足 Send + Sync,B5 删
fn _assert_app_state_send_sync() {
    fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<AppState>();
}
```

如果 `AppState` 任何字段不满足 Send/Sync/'static,这个函数编译报错,你能在 B4 就发现问题,不用拖到 B5 才被 `app.manage` 卡住。编译通过 = 三个约束都过关。

---

## 10. 卡点 / 易错点提醒

- **`tokio::sync::RwLock` import 路径** —— 是 `tokio::sync::RwLock`,不是 `tokio_util`(常见混淆)
- **`CancellationToken` import 路径** —— 是 `tokio_util::sync::CancellationToken`,B1 已装 `tokio-util` crate
- **`reqwest::Client` 不要再包 Arc** —— 它内部已经是 `Arc<...>`,clone 廉价,直接放字段
- **`sqlx::SqlitePool` 同理** —— 内部也是 Arc,不要外面再包
- **`AppState` 自己不要 `Arc<AppState>`** —— `app.manage(state)` 接 `T` 直接 own,Tauri 内部会处理共享
- **字段顺序无所谓** —— Rust struct 字段定义顺序不影响内存布局(编译器会重排),按可读性排即可
- **`unimplemented!()` 不会让编译失败** —— 它是个有 `!`(never type)返回类型的宏,合法占位

---

## 11. 写完贴给我 review 时,我会重点看

- 4 个字段类型对不对(`RwLock<Option<PathBuf>>` / `Option<SqlitePool>` / `reqwest::Client` / `RwLock<HashMap<String, CancellationToken>>`)
- import 路径(`tokio::sync::RwLock` 而不是 `tokio_util::sync`)
- 没有多余 `Arc` 包装
- `lib.rs` 是否同时加了 `mod state;` 和 `pub use state::AppState;`
- 进阶自检函数有没有写(可选)
- `cargo check` 输出
- 你 db 选了哪个方案,有没有记到主进度文档
