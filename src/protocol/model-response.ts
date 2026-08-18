import type { ReasoningEffort } from "./model-provider";

export interface StreamRequest {
  providerId: string;
  modelId: string;
  reasoningEffort: ReasoningEffort | null;
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
