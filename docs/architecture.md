# 项目规格说明书：Electron + Go 本地 AI Agent 桌面应用

> **文档用途**：本项目开发指南及 AI 辅助编程上下文依据。
> **核心架构**：Electron (前端) + Golang (后端 Sidecar)
> **设计原则**：Local-First (本地优先)、单二进制分发、隐私安全、易学习性。

---

## 1. 项目概述 (Project Overview)

本项目旨在开发一个运行在用户桌面的 AI Agent 应用程序。核心逻辑运行在本地，确保用户数据隐私。前端负责交互界面，后端负责业务逻辑、AI API 调用及本地数据存储。

- **项目类型**：桌面客户端 (Desktop Application)
- **运行环境**：Windows / macOS / Linux
- **网络依赖**：需联网调用大模型 API (本地无大模型推理)
- **数据存储**：本地 SQLite 数据库
- **开发目标**：学习 Electron 与 Go 交互架构，实现 AI Agent 核心功能。

---

## 2. 技术栈选型 (Tech Stack)

| 模块               | 技术选型             | 版本/备注     | 选择理由                                       |
| :----------------- | :------------------- | :------------ | :--------------------------------------------- |
| **前端框架** | **Electron**   | Latest Stable | 生态成熟，资料最多，UI 开发效率高。            |
| **前端 UI**  | React + Vite         | Latest        | 开发体验好，组件生态丰富。                     |
| **后端语言** | **Golang**     | 1.21+         | 编译为单二进制，并发好，资料多，适合本地服务。 |
| **Web 框架** | **Gin**        | Latest        | 性能足够，中间件丰富，SSE 支持好，中文文档多。 |
| **数据库**   | **SQLite**     | via GORM      | 单文件存储，无需服务，适合本地应用。           |
| **通信协议** | **HTTP + SSE** | -             | 前端通过 EventSource 接收 AI 流式响应。        |
| **打包工具** | electron-builder     | Latest        | 支持 Sidecar 模式，配置灵活。                  |
| **构建脚本** | Makefile / npm       | -             | 自动化编译 Go 与打包 Electron。                |

---

## 3. 项目目录结构 (Directory Structure)

采用 Monorepo 结构，前端与后端在同一仓库，物理目录分离。

project-root/
├── .gitignore
├── Makefile                  # 统一构建脚本
├── README.md
├── backend/                  # Golang 后端源码
│   ├── cmd/
│   │   └── server/
│   │       └── main.go       # Go 入口文件
│   ├── internal/
│   │   ├── handler/          # HTTP  handlers
│   │   ├── service/          # 业务逻辑 (AI, Tools)
│   │   ├── model/            # 数据库模型
│   │   └── database/         # SQLite 连接
│   ├── go.mod
│   └── go.sum
├── app/                      # Electron 前端源码
│   ├── src/
│   │   ├── main.js           # Electron 主进程 (关键：启动 Go)
│   │   ├── preload.js
│   │   └── renderer/         # React 代码
│   ├── resources/
│   │   └── bin/              # 存放编译后的 Go 二进制 (gitignore)
│   ├── package.json
│   └── electron-builder.yml  # 打包配置
└── scripts/                  # 辅助脚本

---

## 4. 核心架构设计 (Architecture)

### 4.1 进程模型 (Sidecar Pattern)

1. **Electron Main Process**: 负责窗口管理、生命周期管理、**启动/杀死 Go 进程**。
2. **Go Process**: 作为独立二进制运行，监听本地端口 (如 `127.0.0.1:8080`)，提供 HTTP API。
3. **Electron Renderer Process**: 负责 UI 渲染，通过 `fetch` 或 `EventSource` 请求 Go 服务。

### 4.2 通信流程

1. 用户启动应用 -> Electron Main 进程启动。
2. Electron Main 通过 `child_process.spawn` 拉起 `resources/bin/agent`。
3. Go 服务启动，监听端口。
4. 前端界面加载，通过 `http://127.0.0.1:8080` 发起请求。
5. AI 响应通过 **SSE (Server-Sent Events)** 流式推送到前端。
6. 用户关闭应用 -> Electron Main 进程捕获 `will-quit` -> `kill` Go 进程。

---

## 5. 关键实现规范 (Implementation Guidelines)

### 5.1 后端 Go 规范

- **框架**: 必须使用 `Gin`。
- **跨域**: 必须配置 CORS 中间件，允许 Electron 源访问。
- **流式输出**: AI 接口必须使用 `c.SSEvent` 或手动设置 `text/event-stream`  header。
- **数据库**: 使用 `GORM` 连接 SQLite，数据库文件存放在用户数据目录 (`os.UserConfigDir`)。
- **安全**: API Key 必须使用 `go-keyring` 存入系统钥匙串，禁止明文存储。
- **端口**: 开发环境固定 `8080`，生产环境建议动态端口或通过文件握手。

### 5.2 前端 Electron 规范

- **主进程 (main.js)**:
  - 必须实现 `spawn` 逻辑启动 Go 二进制。
  - 必须实现 `app.on('will-quit')` 清理 Go 进程。
  - 路径判断：区分开发环境 (`__dirname`) 与生产环境 (`process.resourcesPath`)。
- **通信**:
  - 禁止在渲染进程直接调用系统 API，必须通过 IPC 或 HTTP。
  - 推荐使用 HTTP 请求 Go 后端，而非 IPC 直接通信 (解耦)。
- **安全**: 开启 `contextIsolation: true`，关闭 `nodeIntegration`。

### 5.3 端口配置

- **开发环境**: 固定 `8080`，启动前检查占用，被占用则报错提示
- **生产环境**: 动态端口（绑定 `0` 随机分配），Go 启动后将实际端口写入 stdout 或临时文件，Electron 主进程读取后建立连接

### 5.4 API Key 存储策略

POC 阶段与生产阶段采用不同策略：

| 阶段 | 存储方式 | 说明 |
| :--- | :--- | :--- |
| **POC** | 环境变量 | 开发最方便，优先读取 `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` |
| **生产** | 系统钥匙串 | 使用 `go-keyring` 存入 OS 钥匙串，禁止明文存储 |

过渡方案：代码先支持环境变量，后续封装成配置模块，无缝切换到钥匙串。

### 5.5 数据库规范 (SQLite)

- **初期**: 仅使用 SQLite 存储聊天记录、配置、用户信息。
- **检索**: 优先使用 SQLite FTS5 进行全文检索。
- **向量**: 暂不引入独立向量数据库。如需语义检索，将 Embedding 存为 BLOB，用 Go 代码计算相似度。

---

## 6. 构建与打包流程 (Build & Deploy)

### 6.1 开发环境启动

# 终端 1: 启动 Go 后端

cd backend && go run cmd/server/main.go

# 终端 2: 启动 Electron 前端

cd app && npm install && npm run dev

### 6.2 开发调试方案

POC 阶段推荐**独立启动**（调试友好）：

```bash
# 终端 1: Go 后端（带热重载可用 air）
cd backend && go run cmd/server/main.go

# 终端 2: Electron 前端
cd app && npm run dev
```

**环境区分：**
- 开发环境：前端直连 `http://127.0.0.1:8080`
- 生产环境：前端通过 `__dirname` / `process.resourcesPath` 找 Go 二进制

**调试技巧：**
- Go 端：用 `log.Println()` 输出，终端直接查看
- Electron 端：主进程在终端查看，渲染进程开 DevTools (F12)
- 联调问题：检查端口占用、`Access-Control-Allow-Origin` 配置

### 6.3 POC 阶段目录结构

POC 阶段保持最小化，避免过度设计：

```
project-root/
├── backend/
│   ├── main.go           # 仅实现 /chat 流式接口
│   └── go.mod
├── app/
│   ├── src/
│   │   ├── main.js       # 仅实现启动 Go + 窗口管理
│   │   └── renderer/     # 仅一个聊天页面 (React)
│   └── package.json
└── Makefile              # 一键构建
```

后续逐步扩展：
- `backend/internal/` → 按 handler/service/model 拆分
- `app/src/renderer/` → 增加组件、状态管理

### 6.4 生产环境打包 (自动化)

必须通过脚本串联编译与打包过程。

# Makefile 示例

.PHONY: build

build:
 # 1. 编译 Go 二进制到前端资源目录
 @echo "Building Go..."
 go build -o app/resources/bin/agent backend/cmd/server/main.go

    # 2. 构建前端网页
 @echo "Building Web..."
 cd app && npm run build

    # 3. 打包 Electron
 @echo "Packaging Electron..."
 cd app && npx electron-builder --publish never

### 6.5 electron-builder 配置

# app/electron-builder.yml

extraResources:

- from: resources/bin/agent
  to: bin/agent
  filter:
  - "**/*"

---

## 7. 约束与禁止事项 (Constraints & Rules)

> **AI 辅助编程时请严格遵守以下规则：**

1. **禁止使用 Rust**: 本项目锁定 Golang 作为后端语言，以确保文档丰富度和开发效率。
2. **禁止引入独立向量数据库**: 如 Chroma, Milvus, Qdrant。本地存储仅限 SQLite。
3. **禁止云端业务逻辑**: 核心业务逻辑必须在本地 Go 后端，云端仅作为 AI 模型提供商 (API)。
4. **禁止硬编码密钥**: 生成代码时，涉及 API Key 必须调用钥匙串接口或环境变量，不可写死在代码中。
5. **禁止混合进程通信**: 优先使用 HTTP/SSE，除非性能瓶颈明显，否则不使用 Stdin/Stdout 通信。
6. **保持单二进制分发**: 最终安装包不应依赖用户预先安装 Go 环境或数据库服务。

---

## 8. 常见问题解决方案 (FAQ)

| 问题                                | 解决方案                                                 |
| :---------------------------------- | :------------------------------------------------------- |
| **Electron 找不到 Go 二进制** | 检查 `process.resourcesPath` (生产) 与 `__dirname` (开发) 的路径拼接是否正确 |
| **Go 进程残留** | Electron 崩溃时可能未触发 `will-quit`，建议启动时 `kill` 同名旧进程 |
| **端口被占用** | 开发时检查 8080 是否被其他服务占用，生产时使用动态端口避免冲突 |
| **跨域错误** | 确保 Go 配置了 CORS，允许 `http://localhost` 或 `file://` 协议访问 |
| **SSE 无输出** | 检查 Go 是否正确设置 `Content-Type: text/event-stream` 和 `Flush` |
