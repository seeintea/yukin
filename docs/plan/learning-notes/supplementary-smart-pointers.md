# 补充 — Rust 智能指针速查表(`Box` / `Rc` / `Arc` / `RefCell` / `Cell` / `Mutex` / `RwLock`)

> 创建日期: 2026-06-08
> 用途: 学习过程中遇到这些类型时回来查。**不是按顺序读的概念课**,是查表。
> 触发场景:在 B4 学并发模型时,初次接触这一组类型,先备着,后面真用到再回头看具体条目。

---

## 一句话速查表

| 类型 | 一句话 | 什么时候用 | JS 类比 |
|------|--------|-----------|---------|
| `Box<T>` | 把 T 放到堆上,**单一所有权** | 想拥有一个具体类型,但要放堆(递归类型、大对象、trait 对象) | `new T()` 然后只有一个变量持有 |
| `Rc<T>` | 单线程引用计数,**多个变量共享** T | 同线程内多个地方共享同一份只读数据 | 多个变量指向同一个 object |
| `Arc<T>` | 跨线程引用计数版的 Rc | 多线程共享同一份数据 | 跨 Worker 共享(JS 没真有,硬比) |
| `RefCell<T>` | **单线程**绕开"借用规则"的内部可变 | 想在 `&self` 方法里改字段,而且不跨线程 | `mutable field on const object`,运行时检查 |
| `Cell<T>` | RefCell 的轻量版,只能整体 get/set,不能拿借用 | 小值(i32、bool)的内部可变 | 同上 |
| `Mutex<T>` | 跨线程互斥锁 | 多线程共享 + 要能改 | Lock |
| `RwLock<T>` | 跨线程读写锁,多读单写 | 同上,但读远多于写 | RWLock |

---

## 经典组合(覆盖 90% 场景)

1. **`Arc<T>`** —— 多线程共享只读数据(T 不可变)
2. **`Arc<Mutex<T>>`** —— 多线程共享可变数据
3. **`Arc<RwLock<T>>`** —— 多线程共享可变数据,读多写少

---

## 选择决策树(从你想干什么倒推)

```
我想把数据放堆上,只有一个持有者?               → Box<T>
我想多个地方共享同一份数据?
  └ 同一个线程?                                  → Rc<T>
  └ 跨线程?                                       → Arc<T>
我想通过 &self 改字段(不要 &mut self)?
  └ 同一个线程?
      ├ 大对象/需要拿借用?                       → RefCell<T>
      └ 小值/整体替换?                            → Cell<T>
  └ 跨线程?
      ├ 读写差不多频繁?                           → Mutex<T>
      └ 读远多于写?                               → RwLock<T>
我既想共享又想能改(跨线程)?
  └ Arc + Mutex/RwLock 组合
```

---

## async 场景的额外规则

**async 代码(用 tokio 等 runtime)**:
- `Mutex` / `RwLock` 用 **`tokio::sync::*`**,**不是 `std::sync::*`**
- 跨 `.await` 持锁:用 tokio 的就安全,用 std 的会死锁
- 详见 [B4 概念课第 4 节](./B4-state-and-concurrency.md)

**sync 代码(普通函数,无 async)**:
- 用 `std::sync::Mutex` / `std::sync::RwLock`
- tokio 的版本反而不能用(它要在 runtime 内)

---

## 我们项目里到底用了哪些?

到 Phase B 为止:

- `tokio::sync::RwLock<...>` — `AppState.workspace` / `AppState.runs`(B4)
- `Arc<dyn Tool>` — `ToolRegistry` 里存 trait 对象(B3 已用,但你可能没注意)
- `reqwest::Client` 和 `sqlx::SqlitePool` 内部已经是 `Arc`(你不用看见)
- `CancellationToken` 内部也是 `Arc`(你 clone 它就是增引用计数)

`Rc` / `RefCell` / `Cell` / `Box` 在我们这个 Tauri 项目里**几乎用不到**(全是 async + 多线程)。**它们活在单线程的 CLI 工具或者 GUI 主线程内部**。

后面 Phase G(agent loop)可能会出现:
- `Arc<RwLock<HashMap<String, ToolDef>>>` — 真正 spawn task 共享状态时
- `Arc<dyn LlmProvider>` — provider 共享给多个并发请求

---

## 易混点

- **`Box<T>` ≠ `Rc<T>`** —— Box 只有一个所有者,Rc 可以多个共享
- **`Rc<T>` ≠ `Arc<T>`** —— 表面行为一样,区别只在"原子操作 vs 非原子",**单线程用 Rc 性能稍好,多线程必须 Arc**。**编译器会强制**(`Rc` 不是 Send,跨线程编译就过不了)
- **`RefCell<T>` ≠ `Mutex<T>`** —— 前者单线程,违规 panic;后者跨线程,加锁
- **不要套娃**:`Rc<Arc<T>>` 没意义,`Box<Arc<T>>` 没意义。组合是有目的的(Arc 共享 + 内部 Mutex 互斥),不是越多越好

---

## 进一步学习

需要彻底理解时:
- [Rust Book Ch.15 — Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- [Rust Book Ch.16 — Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Jon Gjengset — Crust of Rust: Smart Pointers and Interior Mutability](https://www.youtube.com/watch?v=8O0Nt9qY_vo)(YouTube,深度讲)
