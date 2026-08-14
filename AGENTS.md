# Yukin 开发约定

本项目处于早期迭代阶段。实现需求时遵循以下约定，后续可随项目发展继续调整。

## 技术要求

- 以熟练掌握 Rust、React 和 TypeScript 为前提进行设计、实现与代码审查。
- Rust 负责 Tauri 后端、Agent Runtime、系统能力和持久化；React/TypeScript 负责界面与用户交互。除非需求明确变化，保持这一职责边界。
- 优先使用语言和现有项目能力直接解决问题，不因个人偏好引入新的框架、库或基础设施。

## 实现原则

- 选择满足当前需求的最精简方案。
- 只实现已明确需要的能力，不提前设计尚未提出的扩展点。
- 避免过度抽象、过度封装、无实际需求的兼容层和冗余兜底。
- 不为假设性的异常编写大量防御代码；保留编译、运行和当前功能所必需的校验与错误处理。
- 优先保持代码直观、局部且易于修改；仅在重复或复杂度已经真实出现时提取抽象。
- 修改应聚焦当前任务，不顺带重构无关代码。

## Rust 后端结构

- Rust 后端按职责组织代码，不要求所有功能套用统一的 Service、Repository 或领域层模板。
- `commands` 是 Tauri IPC 入口，只负责接收请求、获取应用状态、调用对应能力并返回结果，不编写 SQL 或复杂业务流程。
- `protocol` 只定义 Rust 与 TypeScript 之间传输的请求、响应、事件载荷和可序列化枚举。
- `storage` 只负责 SQLite、SQLx 查询、数据库记录映射和迁移相关实现，不承载跨模块业务流程。
- 简单 CRUD 允许直接使用 `commands -> storage`，不增加无实际逻辑的中间层。
- 当一个操作需要组合多个 Storage、Agent 或系统能力时，再建立具体的工作流模块；工作流负责执行顺序、错误处理和事务边界。
- `agent` 只用于真正的 Agent Runtime、模型调用循环、上下文和工具执行，不用于存放 Run、Message 等数据库 CRUD。
- Rust 内部模块不得调用其他模块的 Tauri Command；Command 仅面向前端 IPC。
- 不提前创建空模块。模块和抽象应随真实功能自然产生。
- 应用级基础设施放在职责明确的模块中，例如数据库连接放在 `storage::database`，日志和诊断放在 `diagnostics`。

## Rust 命名约定

- Tauri Command 使用带业务前缀的全局唯一名称，例如 `model_provider_create`、`model_provider_find`。
- Command 请求类型使用 `Request` 后缀，不使用 `Input`，例如 `CreateRequest`、`FindRequest`、`UpdateRequest`。
- 无请求参数的 Command 不创建空 Request 类型。
- 类型位于明确的业务模块中时，依靠模块路径表达上下文，避免在每个类型名中重复完整业务前缀。
- SQLx 查询记录使用 `Record` 后缀；集合变量使用复数名称，例如 `records`。
- 公共数据库记录信息使用 `RecordMetadata`；具体记录的 ID 保留在对应类型中，不放入公共 Metadata。
- Rust 字段使用 `snake_case`，IPC JSON 字段使用 `camelCase`，可序列化枚举值使用 `snake_case`。
- 数据库表使用复数名称，Migration 使用递增编号和明确动作，例如 `0001_create_model_providers.sql`。
- 新增或修改 SQLx 编译期查询后，同步更新并提交 `.sqlx` offline metadata。

## 非目标

- 不需要考虑无障碍支持。
- 不需要考虑旧浏览器、旧操作系统、旧运行时或历史版本兼容性。
- 不添加 polyfill、兼容分支或降级实现，除非任务明确要求。

## 质量基线

- TypeScript 保持类型明确，避免无必要的 `any`。
- Rust 代码应通过格式化和编译检查，不随意使用会掩盖实际错误的 `unwrap`；在状态确定且上下文清晰时可以使用。
- Rust 改动至少运行 `cargo fmt --all -- --check` 和 `cargo test`；适用时运行 `cargo clippy --all-targets -- -D warnings`。
- React 组件保持职责清晰；简单局部状态不引入全局状态方案。
- 测试与验证应与改动风险相称，不为简单实现搭建庞大的测试体系。
- 完成改动后至少运行与改动直接相关的格式化、类型检查、编译或测试命令。

## Git 提交规范

- 提交信息使用 Conventional Commits 格式，首行为 `<type>: <summary>`；需要明确作用域时可使用 `<type>(<scope>): <summary>`。
- `type` 根据改动性质选择 `feat`、`fix`、`docs`、`refactor`、`test`、`chore`、`build`、`ci`、`perf` 或 `style`。
- 标题和正文默认使用英文；标题应概括整个 PR 或提交的核心改动，不使用句号结尾。
- 标题后空一行，以精简的 `- ` 项目符号分别总结本次工作中不同的改动；每项只概括一个改动及其结果，不展开实现细节，也不重复标题。
- 多个项目符号可以属于同一个核心需求，也可以记录随该需求一并完成的其他独立需求；不要把同一改动的实现步骤拆成多个项目符号。
- 创建提交前检查全部暂存改动，确保一个提交只包含同一目的的变更，并运行与改动直接相关的验证命令。
