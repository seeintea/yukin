# Yukin Imported Skill Runtime 接入计划

> 创建时间：2026-08-24（Asia/Shanghai）
>
> 状态：待实施
>
> 前置成果：本地 Skill 已支持从目录或 ZIP 导入、托管副本、内容摘要、启停、软删除和设置页管理；Agent Runtime 当前仍只使用内置 Skill。

## 1. 目标

让用户导入并启用的 `SKILL.md` 可以在聊天中被选择，并在 Agent Run 创建时经过完整复验后作为系统指令的一部分生效，同时保持现有工具白名单、目录授权和 Tool Call 审批边界不变。

完成后应形成以下闭环：

1. 用户导入并启用本地 Skill。
2. 聊天 Skill Selector 展示内置 Skill 和已启用的 Imported Skill。
3. 用户选择 Imported Skill 并发起消息。
4. Rust 从应用托管副本加载并复验 `SKILL.md`。
5. Agent 使用该 Skill 的指令运行，但只能调用 Runtime 明确允许的工具。
6. Run 持久化 Skill ID 和内容摘要，历史结果可追溯到实际执行版本。

## 2. 当前状态与真实缺口

### 已具备

- `imported_skills` 已保存稳定 UUID、名称、描述、来源、托管路径、内容摘要和启用状态。
- 导入流程限制 1,024 个条目和 128 MiB，拒绝路径穿越、符号链接和特殊文件。
- 目录或 ZIP 导入失败时会清理 staging，数据库写入失败时会清理托管副本。
- 内置 `SkillRegistry` 已实现 Skill 选择、指令拼接、工具限制和 `run_skills` 版本快照。
- Agent Run 已在 Rust 中校验允许工具，文件工具仍受每次 Run 的附件或目录授权约束。

### 尚未具备

- `agent_skill_list` 只返回编译期内置 Skill，不查询 `imported_skills`。
- Skill Selector 无法区分或选择 Imported Skill。
- Agent Run 不读取托管 `SKILL.md`，`enabled` 当前只影响设置页显示。
- Imported Skill 尚无明确的工具声明格式和默认工具策略。
- Run 尚未使用 `content_digest` 作为 Imported Skill 的执行版本。
- 托管副本被篡改、丢失或软删除时缺少 Run 前失败语义。

## 3. 设计决策与安全边界

### 3.1 Skill 身份与版本

- 内置 Skill 继续使用现有稳定字符串 ID 和显式版本号。
- Imported Skill 使用数据库 UUID 作为 ID，不使用可重名的 frontmatter `name` 作为身份。
- Imported Skill 的 `content_digest` 作为 `RunSkill.version`，不新增重复的 Run 快照表。
- 同一 Run 只使用准备阶段加载的指令和摘要；运行中启停或删除不改变已启动 Run。

### 3.2 托管内容是唯一执行来源

- Runtime 不回读用户最初选择的目录或 ZIP，只读取应用数据目录中的托管副本。
- 每次准备 Run 时重新确认记录未删除、处于启用状态、托管路径属于 `app_data_dir/skills/<id>`。
- 在阻塞任务中查找唯一 `SKILL.md`、限制读取大小、重新计算目录摘要并与数据库比较。
- 托管副本缺失、结构变化或摘要不一致时拒绝启动 Run，不静默使用旧内容。
- Runtime 和错误响应不向模型或前端暴露托管绝对路径。

### 3.3 指令解析

- `SKILL.md` 必须继续包含 `name` 和 `description` frontmatter。
- frontmatter 之后的 Markdown 正文作为 Skill instructions；空正文视为无效。
- Skill instructions 设置独立大小上限，建议第一版为 64 KiB。
- 第一版只解析当前真实需要的字段，不实现完整 Skill 编辑器、资源自动加载或递归引用协议。
- Skill 指令位于 Yukin 基础系统指令之后，并明确其不能覆盖工具授权、审批和路径安全规则。

### 3.4 工具策略

- Imported Skill 不因正文提到某个工具而自动获得工具权限。
- frontmatter 新增可选工具声明；只接受 Tool Registry 中存在且对当前 Run 可用的工具名。
- 未声明工具时，Imported Skill 默认不增加通用工具权限。
- 用户显式附加文件或目录后，现有附件/目录授权工具仍按 Run 授权加入，不由 Skill 自行扩大范围。
- 未知工具、重复工具或当前不可用工具在 Run 准备阶段返回明确错误。
- 任何写工具继续使用现有 `write + always approval + arguments digest` 机制。

### 3.5 范围控制

- 第一版保持聊天中单选 Skill，不增加多选 UI。
- 不接入 MCP Server，不让 Imported Skill 声明或启动外部进程。
- 不执行 Skill 包中的脚本，不加载任意动态代码，不开放 Shell。
- 不实现自动 Skill 路由、在线市场、更新检查或原目录同步。

## 4. 切片 0：合并后基线验收

### 任务

- [ ] 运行数据库 Migration 并确认 `0010` 可在空库和现有库上应用。
- [ ] 运行 Rust 格式化、测试和 Clippy，运行前端 lint、格式检查和构建。
- [ ] 桌面端验证目录 Skill、ZIP Skill、合法 MCPB 的导入、启停和删除。
- [ ] 验证非法 ZIP 路径、符号链接、重复名称、缺失 manifest 或 `SKILL.md` 的错误提示。
- [ ] 确认删除托管副本不会影响用户最初选择的源目录或压缩包。

### 出口条件

合并进来的存储管理能力可以独立稳定运行，现有文件与 Agent Run 功能没有回归。

## 5. 切片 1：Imported Skill Runtime Loader

### Rust

- [ ] 在 `storage::imported_skill` 增加按 ID 查询已启用记录所需的最小接口，Storage 只返回数据库记录。
- [ ] 在 Imported Skill workflow 或具体 loader 模块中实现托管根目录、记录 ID、软删除和启用状态复验。
- [ ] 在阻塞任务中查找唯一 `SKILL.md`、读取正文、重新计算摘要并返回 Runtime 定义。
- [ ] 将 frontmatter 解析从“仅导入元信息”扩展为 Runtime 所需的名称、描述、工具声明和正文。
- [ ] 对托管路径越界、内容缺失、空正文、摘要变化、超限正文和无效工具声明返回稳定错误码。
- [ ] 保持错误信息不包含本地绝对路径。

### 测试

- [ ] 覆盖合法托管 Skill 加载与正文提取。
- [ ] 覆盖禁用、软删除、未知 ID、托管目录缺失和路径越界。
- [ ] 覆盖摘要不一致、多个 `SKILL.md`、空正文和超限正文。
- [ ] 覆盖工具声明去重、未知工具和当前不可用工具。

### 出口条件

Rust 可以从一个已启用的 Imported Skill ID 得到经过复验的指令、允许工具和内容摘要，任何托管内容变化都会在 Run 前被拒绝。

## 6. 切片 2：统一 Skill Catalog 与选择器

### Protocol / Rust

- [ ] 扩展 Skill Metadata，增加 `sourceKind` 等 UI 真正需要的来源信息，内置与导入 Skill 使用同一返回类型。
- [ ] 将 `agent_skill_list` 改为异步 Command，通过数据库合并内置 Skill和已启用 Imported Skill。
- [ ] Catalog 只返回可选择的启用项，不返回托管路径、Skill 正文或其他本地文件信息。
- [ ] 明确内置与 Imported Skill 的排序和同名展示策略；身份始终以 ID 判断。

### React

- [ ] Skill Selector 展示内置和导入来源标识，继续保持单选。
- [ ] Imported Skill 被停用或删除后，使 Skill Catalog query 失效并刷新聊天选择器。
- [ ] 当前已选 Skill 消失时清空选择，并在发送前避免提交过期 ID。
- [ ] 设置页文案从“未接入 Runtime”更新为真实状态说明。

### 测试

- [ ] 覆盖 Catalog 合并、仅返回启用项、稳定 ID 和确定性排序。
- [ ] 覆盖启停或删除后的前端缓存刷新。
- [ ] 覆盖选择项失效后回退到通用模式。

### 出口条件

用户可以在聊天中看到并选择已启用的 Imported Skill；停用和删除会及时反映到选择器。

## 7. 切片 3：接入 Agent Run

### Rust

- [ ] 将 Skill 解析改为支持内置定义和 Imported Runtime 定义，但不把数据库或文件读取放入纯 Tool Registry。
- [ ] 在 `agent_run::prepare` 中根据请求 ID 加载 Skill，并在创建 Run 前完成全部复验。
- [ ] 将 Imported Skill 正文追加到基础系统指令，同时保留 Yukin 安全边界的最高优先级说明。
- [ ] 用 Skill 声明工具与当前 Run 可用工具求交集；附件和目录授权仍由现有 workflow 显式加入。
- [ ] 将 Imported Skill UUID 与 `content_digest` 写入现有 `run_skills`。
- [ ] 保持未知、禁用、已删除、摘要变化和工具不可用时 Run 零副作用。
- [ ] 确认 Run 启动后 Skill 状态变化不影响已经准备好的 messages 和 allowed tools。

### React

- [ ] 发送请求继续只提交 Skill ID，不提交指令正文、托管路径或摘要。
- [ ] Run 快照和历史 Tool Call 保持现有展示；适用时增加 Skill 来源或版本摘要展示。
- [ ] 为 Skill 已停用、内容变化和工具不可用提供可理解的错误提示。

### 测试

- [ ] 覆盖 Imported Skill 指令进入首条 System Message。
- [ ] 覆盖未声明工具不能被模型调用。
- [ ] 覆盖写工具仍要求审批，文件工具仍需要当前 Run 的文件或目录授权。
- [ ] 覆盖 `run_skills` 持久化 Imported Skill ID 与内容摘要。
- [ ] 覆盖准备后停用或删除不改变运行中快照。
- [ ] 覆盖伪造 ID、摘要变化和直接绕过前端时 Rust 仍拒绝启动。

### 出口条件

Imported Skill 可以真实影响 Agent 行为，但不能扩大 Runtime 工具权限或绕过现有审批与文件授权。

## 8. 切片 4：桌面端真实模型验收

### 场景

- [ ] 导入一个只改变回答格式、不需要 Tool 的 Skill，并确认模型按其正文回答。
- [ ] 导入一个声明 `current_time` 的 Skill，并确认模型调用只读工具。
- [ ] 导入一个声明写工具的 Skill，确认执行前仍出现参数完整的审批卡片。
- [ ] 选择需要文件工具的 Skill，但不附加目录，确认工具不可用且无文件副作用。
- [ ] 附加目录后重新运行，确认文件工具只能访问该次 Run 的授权范围。
- [ ] 篡改应用托管副本后运行，确认摘要复验失败且错误不泄露绝对路径。
- [ ] 在选择后停用或删除 Skill，确认新 Run 拒绝过期选择。
- [ ] 重启应用后确认 Skill Catalog、启用状态和历史 Run Skill 快照仍一致。

### 出口条件

完成“导入 → 启用 → 选择 → 运行 → 工具约束 → 版本审计”的真实桌面闭环，且现有文件操作和内置 Skill 无回归。

## 9. MCP 后续边界

Imported Skill Runtime 验收完成后，再建立独立 MCP Runtime 计划，顺序建议为：

1. MCP 用户配置与 Keychain 凭据存储。
2. 配置完整性、运行时可用性和健康状态。
3. 单个托管 stdio Server 的启动、初始化、`tools/list` 和退出。
4. Tool namespace、超时、输出限制、进程回收与崩溃处理。
5. MCP Tool 风险分类、逐次审批和 Tool Call 审计。
6. Imported Skill 声明 MCP Tool 的绑定规则。

在上述能力完成前，MCP 的 `enabled` 只代表用户配置意图，不代表 Server 已运行或 Tool 已可用。

## 10. 建议提交粒度

```text
feat(skills): add verified imported skill loader
feat(skills): expose imported skills in agent catalog
feat(skills): resolve imported skills for agent runs
test(skills): cover imported skill runtime boundaries
docs(skills): record desktop runtime acceptance
```

每个提交前至少运行：

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `pnpm oxc`
- `pnpm build`
- 涉及 Migration 时更新并提交 `.sqlx` offline metadata
