import type { ConversationMessage } from "./conversation";
import type { ReasoningEffort } from "./model-provider";

export interface AgentRunStartRequest {
  conversationId: string;
  providerId: string;
  modelId: string;
  reasoningEffort: ReasoningEffort | null;
  content: string;
}

export interface AgentRunStartResponse {
  runId: string;
  userMessageId: string;
  assistantMessageId: string;
}

export type RunStatus =
  | "pending"
  | "running"
  | "waiting_approval"
  | "completed"
  | "failed"
  | "cancelled";

export interface AgentRun {
  id: string;
  conversationId: string;
  providerId: string;
  modelId: string;
  reasoningEffort: ReasoningEffort | null;
  status: RunStatus;
  errorCode: string | null;
  errorMessage: string | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
}

export interface AgentRunSnapshot {
  run: AgentRun;
  assistantMessage: ConversationMessage;
}

interface AgentRunEventEnvelope<TEvent extends string, TData> {
  schemaVersion: 1;
  eventId: string;
  conversationId: string;
  runId: string;
  sequence: number;
  timestamp: string;
  event: TEvent;
  data: TData;
}

export type AgentRunEvent =
  | AgentRunEventEnvelope<"run_started", { userMessageId: string; assistantMessageId: string }>
  | AgentRunEventEnvelope<"phase_changed", { phase: "thinking" | "responding" }>
  | AgentRunEventEnvelope<"output_text_delta", { content: string }>
  | AgentRunEventEnvelope<
      "usage_updated",
      { promptTokens: number; completionTokens: number; totalTokens: number }
    >
  | AgentRunEventEnvelope<"run_completed", Record<string, never>>
  | AgentRunEventEnvelope<"run_failed", { errorCode: string; errorMessage: string }>
  | AgentRunEventEnvelope<"run_cancelled", Record<string, never>>;
