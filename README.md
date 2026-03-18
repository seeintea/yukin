# yukin

> Electron + Go 本地 AI Agent 桌面应用

## 架构

- **前端**: Electron + React + Vite
- **后端**: Go + Gin (Sidecar 模式)
- **通信**: HTTP + SSE
- **存储**: SQLite

## 开发准备

```bash
# 1. 安装依赖
make init

# 2. 启动开发环境（需要两个终端）
# 终端 1: 启动 Go 后端
make dev-server

# 终端 2: 启动 Electron 前端
make dev-app
```

## 目录结构

```
.
├── backend/          # Go 后端源码
│   └── cmd/server/
├── app/              # Electron 前端源码
│   ├── src/
│   │   ├── main.js       # 主进程
│   │   ├── preload.js    # 预加载脚本
│   │   └── renderer/     # React 渲染进程
│   └── resources/bin/    # Go 二进制存放目录
├── scripts/          # 辅助脚本
├── docs/             # 文档
│   └── architecture.md   # 项目架构规格说明书
└── Makefile          # 构建脚本
```

## 构建

```bash
# 构建生产版本
make build
```

## License

MIT
