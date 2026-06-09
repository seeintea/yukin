# C1 — SQLite schema 设计 + FTS5 trigger(概念课)

> 创建日期: 2026-06-08
> 配套: [phase C 学习总入口](../2026-06-08-phase-c-learning.md) / [phase C 架构定义](../2026-06-06-phase-c-sqlite-keychain-session.md)
> 用途: C1 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

这一步 **Rust 代码量是 0**,但读懂这份 SQL 决定后面 3 步能不能写对。分 7 节讲:

1. 为什么 schema 这一步独立成课
2. SQLite 的 5 个小怪癖(跟 PG/MySQL 不一样)
3. 5 张表逐张过一遍 + 字段语义
4. FTS5 三件套(虚拟表 + 外部内容模式 + tokenizer)
5. FTS5 同步 trigger 模板 + 为什么必须手写
6. 外键 cascade 与 `PRAGMA foreign_keys`(留 hook 给 C2)
7. sqlx migrations 工作流 + `query!` 派的开发环境准备

然后任务、自检、卡点。

---

## 1. 为什么 schema 独立成一节课

普通 CRUD 思维:schema 是个"早晚要建的事",上来就写完。

我们这里不行,因为:

- **错一个字段类型或约束,后面 3 步全要返工**(C3 用 `query!` 编译期校验列名,你 schema 写 `name` 但代码写 `title` 会被编译器直接拒绝)
- **FTS5 trigger 是 SQL 里最容易写错的部分**,而且写错的症状不是报错,是"FTS 查询返回僵尸记录"(删了 memory 但 FTS 还查得到)
- **`foreign_keys` 默认是关的** —— 这是 SQLite 特殊性,C5 的 cascade 测试不挂这条 PRAGMA 一定失败

所以这步用心一点,后面省心。

---

## 2. SQLite 的 5 个小怪癖

写过 PG / MySQL / 前端调过 ORM 的人,看 SQLite 会有 5 个意外:

### ① 动态类型(type affinity)

```sql
CREATE TABLE t (n INTEGER);
INSERT INTO t (n) VALUES ('hello');   -- 这居然能成功!
```

SQLite 的字段类型是"建议",不是"强约束"。你给 `INTEGER` 字段塞字符串它默认接受。**实践中靠 `CHECK` 约束补**,或者在 Rust 端用 sqlx 的强类型映射(`row.get::<i64, _>("n")` 拿不到就 `Err`)。

### ② 没有真正的 `BOOLEAN`

写 `BOOLEAN` 也行,但 SQLite 内部存 `INT`(0 / 1)。我们 schema 里 `providers.has_key INT NOT NULL DEFAULT 0` 就是这个 pattern。Rust 端 sqlx 把 `INT` 自动映射 `bool` 也行,映射 `i64` 也行。

### ③ datetime 用 `TEXT` 存字符串

主流两种方案:
- **`TEXT` 存 ISO 8601 字符串** —— 人类可读,`datetime('now')` 生成 `'2026-06-09 12:34:56'`(注意:没 `T`、没时区)
- **`INTEGER` 存 Unix epoch** —— 紧凑、运算快、跨时区不混淆

phase-c doc 用了第一种(`TEXT NOT NULL DEFAULT (datetime('now'))`),我们跟。**坑预警**:`datetime('now')` 的输出格式跟 chrono 默认 RFC3339 不兼容,C3 写 `MemoryRow` 时会撞上,届时再讲。

### ④ `CHECK` 约束被认真执行

```sql
kind TEXT NOT NULL CHECK (kind IN ('user','feedback','project','reference'))
```

插入 `kind='other'` 会被拒。这是我们 `MemoryKind` enum 在 db 层的"安全网"。

### ⑤ `PRAGMA foreign_keys=ON` 默认关

```sql
CREATE TABLE messages (
  ...
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
);
```

你写了 `REFERENCES`,**它默认根本不生效**!删 sessions 行,messages 不会跟着删。

每个连接必须显式 `PRAGMA foreign_keys=ON`,才会真激活。**C2 必须在 `AppState::new` 里加这句**,这是 phase-c doc 漏写的(我已经标在 C2 的概念课大纲里)。

---

## 3. 5 张表逐张过一遍

phase-c doc 第 11–64 行已经给了完整 SQL。这里讲**为什么这么设计**。

### 3.1 `settings` —— key-value 配置

```sql
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

最简单的 K-V 表。存什么?目前只用一个 key `workspace_path`,后续 Phase E 可能扩(默认 provider、UI 偏好等)。

**为什么不放 JSON 单行?** —— 一旦多 key 并发更新会冲突;K-V 表天然并发友好(主键不同行)。

### 3.2 `providers` —— LLM provider 注册表

```sql
CREATE TABLE providers (
  name TEXT PRIMARY KEY,
  has_key INT NOT NULL DEFAULT 0,
  default_model TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

记录"哪些 provider 已经存了 API key"。**注意 key 本身不在这里,而在 keychain**;这张表只存元数据(有没有 key、用什么默认模型)。

为什么需要这张表?因为 keychain **没有列举 API**(C4 会展开讲)。你想给前端显示"已配置的 provider 列表",必须自己存一份索引 —— 这就是 `providers` 表存在的全部理由。

### 3.3 `memory` —— 用户记忆条目

```sql
CREATE TABLE memory (
  id TEXT PRIMARY KEY,                           -- uuid v4
  name TEXT NOT NULL,                            -- 短标题
  kind TEXT NOT NULL CHECK (kind IN ('user','feedback','project','reference')),
  description TEXT,                              -- 摘要,可空
  content TEXT NOT NULL,                         -- 正文
  metadata TEXT NOT NULL DEFAULT '{}',           -- JSON 字符串,扩展点
  workspace TEXT,                                -- NULL = 全局,有值 = 仅该 workspace
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

字段关键点:

- **`id` 是 TEXT 不是 INTEGER** —— 用 uuid 不用自增 id,跨 workspace 同步 / 导入导出不会撞 id
- **`kind` 用 `CHECK`** —— 4 种合法值,db 层最后一道防线
- **`workspace TEXT NULL`** —— **NULL 表示全局可见**,非 NULL 表示绑定到某 workspace。Rust 端用 `Option<String>` 表达
- **`metadata TEXT` 存 JSON 字符串** —— SQLite 没有原生 JSON 类型(SQLite 3.45+ 有 `JSONB`,但我们不依赖),sqlx 端用 `serde_json::Value` 包,序列化时手动 `to_string`

### 3.4 `sessions` —— 对话会话

```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,                           -- uuid
  title TEXT NOT NULL,
  workspace_path TEXT,                           -- 这个 session 用哪个 workspace
  provider TEXT,                                 -- 这个 session 锁定的 provider
  model TEXT,                                    -- 锁定的 model
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

直白。`provider` / `model` 都可空,允许用户后续切换。

### 3.5 `messages` —— 单条消息

```sql
CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('system','user','assistant','tool')),
  content TEXT NOT NULL,                         -- JSON,Anthropic Messages 格式
  tool_calls TEXT,                               -- JSON,Phase G 填
  tool_results TEXT,                             -- JSON,Phase G 填
  step_index INTEGER,                            -- agent loop 里的第几步
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

关键:

- **`content` 字段存的是 Anthropic Messages API 的 content 格式** —— 不是纯文本,可能是 `[{type:"text", text:"..."}, {type:"image", source:...}]` 这种数组
- **`tool_calls` / `tool_results` 现阶段为 NULL** —— Phase G 才用上
- **FK + CASCADE** —— 删 session 自动删它的 messages(前提:`PRAGMA foreign_keys=ON`)

### 3.6 索引

```sql
CREATE INDEX memory_kind_idx      ON memory(kind);
CREATE INDEX memory_workspace_idx ON memory(workspace);
CREATE INDEX messages_session_idx ON messages(session_id, created_at);
```

- 前两个支撑 `memory_list` 的过滤(by kind / by workspace)
- 第三个是**复合索引**,支撑"按 session 查所有消息,按时间排序"这个常见 query —— SQLite 能直接用这个复合索引走 index scan,不需要排序

---

## 4. FTS5 三件套

FTS5 = SQLite 全文搜索引擎,我们 `memory_recall` 命令的核心。

### 4.1 虚拟表

```sql
CREATE VIRTUAL TABLE memory_fts USING fts5(
  name, description, content,
  content='memory', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2'
);
```

拆解:

- **`VIRTUAL TABLE ... USING fts5(...)`** —— FTS5 是个虚拟表实现,内部用倒排索引,行为像表但不能 ALTER
- **`name, description, content`** —— 索引哪几列(注意是 `memory` 表里这三列的内容)
- **`content='memory'`** —— **外部内容模式**(external content mode):FTS 不自己存数据,只存倒排索引,真数据还在 `memory` 表里。**省空间**,但要求你手写 trigger 同步(下一节)
- **`content_rowid='rowid'`** —— FTS 用 `memory` 表的 `rowid` 做关联键(SQLite 每张表自带一个隐式 `rowid` 列,值是插入顺序的自增整数,跟 `id` UUID 不一样)
- **`tokenize='unicode61 remove_diacritics 2'`** —— 用 `unicode61` 分词器,去重音符(变体 2 = 处理更多 Unicode 字符)。**对中文的效果**:能按字符切,但不能"按词切"(没有 jieba 那种中文分词)。MVP 够用,生产想要更好的中文搜索需要换 `trigram` tokenizer 或外接 jieba

### 4.2 用法预览

```sql
-- 查询
SELECT m.* FROM memory m
JOIN memory_fts f ON f.rowid = m.rowid
WHERE memory_fts MATCH 'hello'
ORDER BY rank LIMIT 8;
```

- **`MATCH` 而不是 `LIKE`** —— `LIKE` 是慢扫,`MATCH` 走倒排索引,毫秒级
- **`ORDER BY rank`** —— FTS5 内置相关性评分,小的(更相关)在前
- **`JOIN m ON f.rowid = m.rowid`** —— 因为外部内容模式,FTS 只有 rowid,要 join 回 `memory` 拿真数据

---

## 5. FTS5 同步 trigger(本节最容易踩坑)

外部内容模式有一个**致命陷阱**:FTS 表的内容**不会自动**跟 `memory` 表同步。

```sql
INSERT INTO memory (id, name, content, ...) VALUES (...);
-- ↑ memory 表新增一行
-- ↓ FTS 索引仍然是空的!
SELECT * FROM memory_fts WHERE memory_fts MATCH 'hello';   -- 0 行
```

所以**你必须手写 3 个 trigger**,把 `memory` 表的写操作同步到 FTS 索引。

### 模板(直接抄)

```sql
-- INSERT 同步:新插入的 memory 行,索引内容
CREATE TRIGGER memory_ai AFTER INSERT ON memory BEGIN
  INSERT INTO memory_fts(rowid, name, description, content)
  VALUES (new.rowid, new.name, new.description, new.content);
END;

-- DELETE 同步:用 FTS5 的特殊 'delete' 命令
CREATE TRIGGER memory_ad AFTER DELETE ON memory BEGIN
  INSERT INTO memory_fts(memory_fts, rowid, name, description, content)
  VALUES('delete', old.rowid, old.name, old.description, old.content);
END;

-- UPDATE 同步:先 'delete' 再 'insert'
CREATE TRIGGER memory_au AFTER UPDATE ON memory BEGIN
  INSERT INTO memory_fts(memory_fts, rowid, name, description, content)
  VALUES('delete', old.rowid, old.name, old.description, old.content);
  INSERT INTO memory_fts(rowid, name, description, content)
  VALUES (new.rowid, new.name, new.description, new.content);
END;
```

**关键点**:

- **trigger 名约定** `<table>_ai` (after insert) / `_ad` (after delete) / `_au` (after update) —— 不是强制,但 FTS5 文档示例就这样
- **`new.xxx` 引用 INSERT/UPDATE 后的新行**,**`old.xxx` 引用 DELETE/UPDATE 前的旧行** —— 必须带前缀,纯写 `rowid` SQLite 不知道你指哪行
- **删除 / 更新用 `'delete'` 关键字** —— FTS5 不能直接 `DELETE FROM memory_fts WHERE rowid = ?`,必须用这种"伪 INSERT 'delete' 命令"模式。这是 FTS5 的特殊 API,记住就好
- **trigger 失败 = 静默错位** —— 如果 trigger 写错没建上,`INSERT INTO memory` 仍然成功,但 FTS 索引不更新。**症状**:你查 `memory_recall` 返回 0 行,但 `SELECT * FROM memory` 明明有数据;或者删了 memory 后 FTS 还查到"幽灵记录"

### 怎么验证 trigger 装上了

```sql
SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name='memory';
-- 应该返回 memory_ai / memory_ad / memory_au 三行
```

---

## 6. 外键 cascade 与 `PRAGMA foreign_keys`

刚才"5 个小怪癖"第 ⑤ 条已经讲了 SQLite **默认外键关闭**。重点强调:

```sql
-- schema 里这样写:
session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
```

```rust
// 但每次打开连接后必须:
sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;
```

**两件事必须都做,缺一不可**。

C1 这一步只写 schema,**PRAGMA 是 C2 的活**。这里给你一个提醒就行,C2 概念课会展开。

---

## 7. sqlx migrations 工作流

### 文件命名约定

```
src-tauri/migrations/
├── 0001_init.sql              ← 我们这一步要建的
├── 0002_add_xxx.sql           ← 未来 schema 改动
└── ...
```

- **数字前缀单调递增**(4 位 / 不超过 i64),用来排序
- **下划线后是描述**(snake_case,人类可读)
- **`.sql` 后缀必须**

### `sqlx::migrate!("./migrations")` 是什么

C2 会调用这个宏:

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

它做 3 件事:

1. **编译期**:把 `migrations/` 目录下所有 `.sql` 文件内容**编进二进制**(发布时不需要带 sql 文件)
2. **运行时**:在 db 里建一张元表 `_sqlx_migrations`,记录已经跑过哪些 migration
3. **运行时**:对比文件列表和 `_sqlx_migrations`,只跑新的

### 路径是相对 `Cargo.toml` 不是源文件

`sqlx::migrate!("./migrations")` 找的是 `<workspace_root>/src-tauri/migrations/` —— 相对 `Cargo.toml` 所在目录。**不是相对 `lib.rs` 所在目录**。

### 我们不做回滚

`sqlx-cli` 支持 `--reversible` 把每个迁移拆成 `up.sql` + `down.sql`,我们**不用**。理由:

- SQLite ALTER TABLE 能力有限(不能改列类型 / 删列在 3.35 前不行),很多 schema 演化在 SQLite 上是"重建表 + 复制数据"的大手术,回滚反而更危险
- 个人/小团队应用,forward-only 简单可靠

如果以后真要改 schema,加新 migration 文件,**不要改老的**(已经跑过的 migration 改了会触发"checksum mismatch"错误)。

### `query!` 派的开发环境准备(**重要,本步骤要做**)

C1 决策选了 `query!` 派 —— `cargo check` 时编译器要连数据库校验 SQL,需要准备:

#### 选 A:开发期连真 db(简单)

```powershell
# 在 src-tauri/ 目录下
cd F:\workspace\open-source\yukin\src-tauri

# 1. 安装 sqlx-cli(Windows 必须带这串 feature flag,默认会拉一堆没用的)
cargo install sqlx-cli --no-default-features --features rustls,sqlite

# 2. 设置 DATABASE_URL(项目根有 .env 文件 sqlx 会自动读)
echo "DATABASE_URL=sqlite:./dev.db" > .env

# 3. 用 sqlx-cli 建库 + 跑迁移
$env:DATABASE_URL = "sqlite:./dev.db"
sqlx database create
sqlx migrate run
```

之后 `cargo check` 会用 `dev.db` 校验所有 `query!` 宏。

**`.env` 文件加进 `.gitignore`**,但 `dev.db` **也加进 `.gitignore`**(每人本地一份)。

#### 选 B:离线模式(跨设备协作友好)

```powershell
# 在已经跑过选 A 一次之后
cargo sqlx prepare
# 会在 src-tauri/ 下生成 .sqlx/ 目录,包含所有 query 的元数据
```

**`.sqlx/` 加进 git**,这样别人 / CI 不需要装 sqlx-cli 也能编译。**每次改了 query!() 或 migration 都要重跑 `cargo sqlx prepare`**。

CI 环境用 `SQLX_OFFLINE=true` 强制走 `.sqlx/` 缓存。

#### 我们怎么选?

**C1 阶段先用选 A**(连真 db),等 C3 真写 `query!` 时再决定要不要上选 B。

**为什么 C1 就要装 sqlx-cli**? —— 因为本步骤验证最后一步要用 sqlite CLI 跑 schema(下一节),sqlx-cli 顺带能用。

---

## 8. 你的任务

### 8.1 准备 sqlx-cli + DATABASE_URL(开始 schema 之前)

```powershell
cd F:\workspace\open-source\yukin\src-tauri

# 装 sqlx-cli(Windows 必须带 feature flag)
cargo install sqlx-cli --no-default-features --features rustls,sqlite

# 建本地开发 db
"DATABASE_URL=sqlite:./dev.db" | Out-File -Encoding utf8 .env
$env:DATABASE_URL = "sqlite:./dev.db"

# .gitignore 加这两条
"dev.db" | Add-Content .gitignore
".env" | Add-Content .gitignore
```

**装完 `sqlx --version` 验证**。装失败回来贴报错。

### 8.2 新建迁移文件

```
src-tauri/migrations/0001_init.sql
```

按 phase-c doc 第 11–64 行那段 SQL 抄,**重点补 3 个 trigger**(doc 里只写了注释占位)。

trigger 模板就用本课第 5 节给的(可以原样抄)。

### 8.3 跑迁移 + 验证

```powershell
# 在 src-tauri/ 下
$env:DATABASE_URL = "sqlite:./dev.db"
sqlx migrate run

# 验证 schema
sqlite3 dev.db ".tables"
# 期望输出含: _sqlx_migrations  memory  memory_fts  messages  providers  sessions
# 还会有 memory_fts_config / memory_fts_data 等 FTS5 内部表(不用管)

# 验证 trigger
sqlite3 dev.db "SELECT name FROM sqlite_master WHERE type='trigger';"
# 期望:memory_ai / memory_ad / memory_au

# 验证 index
sqlite3 dev.db "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%';"
# 期望:memory_kind_idx / memory_workspace_idx / messages_session_idx
```

### 8.4 手动跑通一次 INSERT + FTS 查询(可选但推荐)

```powershell
sqlite3 dev.db
```

进入 sqlite shell 后:

```sql
INSERT INTO memory (id, name, kind, content) VALUES ('test-1', 'hello world', 'user', 'this is some content');
SELECT * FROM memory;
SELECT * FROM memory_fts WHERE memory_fts MATCH 'hello';
-- 如果返回 1 行,说明 trigger 装上了 + FTS 工作正常

DELETE FROM memory WHERE id = 'test-1';
SELECT * FROM memory_fts WHERE memory_fts MATCH 'hello';
-- 如果返回 0 行,说明 DELETE trigger 也工作了

.quit
```

### 8.5 `cargo check`

```powershell
cd F:\workspace\open-source\yukin\src-tauri
cargo check
```

应该通过 —— 本步骤没改任何 Rust 代码,只是确认环境没坏。

---

## 9. 验证清单

- [ ] `sqlx --version` 输出版本号
- [ ] `migrations/0001_init.sql` 存在,5 张表 + FTS5 虚拟表 + 3 个 trigger + 2 个 index 全部就位
- [ ] `sqlx migrate run` 成功(可能输出 "Applied 1/migrate init")
- [ ] `sqlite3 dev.db ".tables"` 看到所有表
- [ ] `sqlite3 dev.db "SELECT name FROM sqlite_master WHERE type='trigger';"` 看到 3 个 trigger
- [ ] (推荐) 手动 INSERT + MATCH 验证 FTS 工作
- [ ] (推荐) DELETE 后 MATCH 返回 0 行,验证 DELETE trigger
- [ ] `cargo check` 通过
- [ ] `dev.db` 和 `.env` 加入 `.gitignore`

---

## 10. 卡点 / 易错点提醒

### sqlx-cli 安装相关

- **`cargo install sqlx-cli` 不加 feature flag 会装 5–10 分钟还可能失败** —— 默认 feature 拉 postgres + mysql + native-tls,Windows 上一定踩链接错。**必须** `--no-default-features --features rustls,sqlite`
- 装完报"command not found":检查 `$env:Path` 含 `%USERPROFILE%\.cargo\bin`
- 用 `cargo install --locked` 锁定版本,避免不同设备装不同版本

### SQL 语法陷阱

- **FTS5 trigger 引用列名必须带 `new.` / `old.` 前缀**,纯写 `rowid` 是错的
- **`'delete'` 命令是字符串字面量**,单引号不是双引号(SQLite 在某些 build 里把双引号当列名)
- **`DEFAULT (datetime('now'))` 必须有外层括号**,纯写 `DEFAULT datetime('now')` SQLite 解析报错
- **`CHECK` 约束的字符串列表用单引号**:`CHECK (kind IN ('user','feedback','project','reference'))`

### FTS5 行为陷阱

- **外部内容模式下,绝对不要直接给 fts 表 INSERT 数据**(`INSERT INTO memory_fts(...) VALUES(...)` 不通过 trigger)。trigger 是唯一入口
- **trigger 装错的症状是"幽灵记录"**:你删了 memory 但 `memory_fts` 还查得到,因为 DELETE trigger 没装上或写错
- 中文搜索效果有限:`unicode61` 只按字符切,搜"机器学习"找不到只含"机器"的记录。MVP 接受

### 迁移工作流

- **改老的 migration 文件会触发 checksum mismatch** —— 新 schema 永远加新文件
- **`sqlx::migrate!()` 路径相对 `Cargo.toml`** —— 写 `"./migrations"` 是对的,写 `"./src/migrations"` 错
- `_sqlx_migrations` 表是 sqlx 自动建的,不要碰

---

## 11. 写完贴给我 review 时,我会重点看

- 3 个 trigger 是否完整(`memory_ai` / `memory_ad` / `memory_au`),`old.` / `new.` 前缀是否对
- `CHECK` 拼写是否正确(`'user','feedback','project','reference'` 一个字符不能错,因为 C3 的 enum 要严格对齐)
- 4 个 index 是否齐(`memory_kind_idx` / `memory_workspace_idx` / `messages_session_idx` —— 算上 FTS5 自动建的内部 index 共 4 个)
- `dev.db` 和 `.env` 是否进了 `.gitignore`
- sqlite CLI 手动跑通 INSERT + MATCH + DELETE + MATCH 的结果截图/输出
- `cargo check` 通过
- `sqlx-cli` 装在哪个版本,有没有踩 Windows feature flag 坑
