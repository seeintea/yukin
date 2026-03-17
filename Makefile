# Yukin 构建脚本
# Electron + Go Local AI Agent

.PHONY: dev build clean init

# 开发命令：分别启动后端和前端（需要两个终端）
dev-backend:
 @echo "Starting Go backend..."
 cd backend && go run cmd/server/main.go

dev-frontend:
 @echo "Starting Electron frontend..."
 cd app && npm run dev

# 安装依赖
init:
 @echo "Installing Go dependencies..."
 cd backend && go mod tidy
 @echo "Installing Node dependencies..."
 cd app && npm install

# 构建生产版本
build:
 @echo "Building Go binary..."
 go build -o app/resources/bin/agent backend/cmd/server/main.go
 @echo "Building web assets..."
 cd app && npm run build
 @echo "Packaging Electron..."
 cd app && npx electron-builder --publish never

# 仅构建 Go 二进制（用于测试 sidecar）
build-go:
 @echo "Building Go binary..."
 go build -o app/resources/bin/agent backend/cmd/server/main.go

# 仅构建前端
build-web:
 cd app && npm run build

# 清理构建产物
clean:
 rm -rf app/dist
 rm -rf app/dist-electron
 rm -f app/resources/bin/agent
 rm -f backend/go.sum
 cd backend && go mod tidy
