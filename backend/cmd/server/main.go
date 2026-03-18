package main

import (
    "log"
    "net/http"
    "os"
    "strings"

    "github.com/gin-contrib/cors"
    "github.com/gin-gonic/gin"
    "github.com/joho/godotenv"
)

func main() {
    // 加载 .env 文件
    if err := godotenv.Load(); err != nil {
         log.Printf("Warning: Failed to load .env file: %v", err)
    }

    r := gin.Default()

    // CORS 配置
    config := cors.DefaultConfig()

    // 从环境变量读取允许的源，默认值为 http://localhost:5173 和 http://127.0.0.1:5173
    allowedOrigins := os.Getenv("ALLOWED_ORIGINS")
    if allowedOrigins == "" {
         config.AllowOrigins = []string{"http://localhost:5173", "http://127.0.0.1:5173"}
    } else {
         // 支持多个源，用逗号分隔
         config.AllowOrigins = strings.Split(strings.TrimSpace(allowedOrigins), ",")
         // 去除每个源的空格
         for i, origin := range config.AllowOrigins {
              config.AllowOrigins[i] = strings.TrimSpace(origin)
         }
    }

    config.AllowMethods = []string{"GET", "POST", "OPTIONS"}
    config.AllowHeaders = []string{"Origin", "Content-Type", "Accept"}
    r.Use(cors.New(config))

    // 健康检查
    r.GET("/health", func(c *gin.Context) {
         c.JSON(http.StatusOK, gin.H{"status": "ok"})
    })

    // TODO: 添加 /chat 流式接口

    // 从环境变量读取端口配置，默认 8080
    port := os.Getenv("PORT")
    if port == "" {
         port = "8080"
    }

    // 确保端口号格式正确（以 : 开头）
    if !strings.HasPrefix(port, ":") {
         port = ":" + port
    }

    log.Printf("Server starting on %s", port)
    if err := r.Run(port); err != nil {
         log.Fatalf("Failed to start server: %v", err)
    }
}
