package main

import (
 "log"
 "net/http"

 "github.com/gin-contrib/cors"
 "github.com/gin-gonic/gin"
)

func main() {
 r := gin.Default()

 // CORS 配置
 config := cors.DefaultConfig()
 config.AllowOrigins = []string{"http://localhost:5173", "http://127.0.0.1:5173"}
 config.AllowMethods = []string{"GET", "POST", "OPTIONS"}
 config.AllowHeaders = []string{"Origin", "Content-Type", "Accept"}
 r.Use(cors.New(config))

 // 健康检查
 r.GET("/health", func(c *gin.Context) {
   c.JSON(http.StatusOK, gin.H{"status": "ok"})
 })

 // TODO: 添加 /chat 流式接口

 log.Println("Server starting on :8080")
 if err := r.Run(":8080"); err != nil {
   log.Fatalf("Failed to start server: %v", err)
 }
}
