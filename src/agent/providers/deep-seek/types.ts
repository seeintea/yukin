export type DeepSeekFinishReason =
  | "stop" // 模型自然停止
  | "length" // 输出长度达到了模型上下文长度限制
  | "content_filter" // 输出内容因触发过滤策略而被过滤
  | "insufficient_system_resource"; // 系统推理资源不足，生成被打断

export type DeepSeekStreamEvent =
  | { type: "content"; content: string }
  | { type: "finish"; reason: DeepSeekFinishReason };
