export interface StreamRequest {
  providerId: string;
  modelId: string;
  content: string;
}

export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export type StreamEvent =
  | {
      event: "output_delta";
      data: { content: string };
    }
  | {
      event: "completed";
      data: {
        finishReason: string | null;
        usage: TokenUsage | null;
      };
    };
