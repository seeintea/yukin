import type { ConversationMessage } from "./conversation";
import type { DirectoryReference, FileReference } from "./file";
import type { ReasoningEffort } from "./model-provider";

export interface AgentRunStartRequest {
  conversationId: string;
  providerId: string;
  modelId: string;
  reasoningEffort: ReasoningEffort | null;
  content: string;
  skillIds: string[];
  attachments: FileReference[];
  directoryScopes: DirectoryReference[];
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
  skills: AgentRunSkill[];
}

export interface AgentRunSkill {
  id: string;
  version: string;
}

export interface AgentRunSnapshot {
  run: AgentRun;
  assistantMessage: ConversationMessage;
  toolCalls: ToolCallSnapshot[];
}

export type ToolCallStatus =
  | "requested"
  | "waiting_approval"
  | "running"
  | "completed"
  | "failed"
  | "rejected"
  | "cancelled";
export type ToolRiskLevel = "read_only" | "write";
export type ToolApprovalPolicy = "never" | "always";

export interface ToolCallSnapshot {
  id: string;
  runId: string;
  name: string;
  arguments: unknown;
  argumentsDigest: string;
  status: ToolCallStatus;
  result: unknown | null;
  riskLevel: ToolRiskLevel;
  approvalPolicy: ToolApprovalPolicy;
  errorCode: string | null;
  errorMessage: string | null;
  approvalExpiresAt: string | null;
  createdAt: string;
  completedAt: string | null;
}

export type ActiveToolCall = ToolCallSnapshot;

export type ToolCallDecision = "allow" | "reject";

export interface ToolCallDecideRequest {
  runId: string;
  toolCallId: string;
  argumentsDigest: string;
  decision: ToolCallDecision;
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
  | AgentRunEventEnvelope<
      "tool_call_requested",
      {
        toolCallId: string;
        name: string;
        arguments: unknown;
        argumentsDigest: string;
        riskLevel: ToolRiskLevel;
        approvalPolicy: ToolApprovalPolicy;
      }
    >
  | AgentRunEventEnvelope<
      "tool_approval_required",
      { toolCallId: string; argumentsDigest: string; expiresAt: string }
    >
  | AgentRunEventEnvelope<"tool_call_started", { toolCallId: string }>
  | AgentRunEventEnvelope<"tool_call_completed", { toolCallId: string; result: unknown }>
  | AgentRunEventEnvelope<
      "tool_call_failed",
      { toolCallId: string; errorCode: string; errorMessage: string }
    >
  | AgentRunEventEnvelope<"tool_call_rejected", { toolCallId: string }>
  | AgentRunEventEnvelope<"run_completed", Record<string, never>>
  | AgentRunEventEnvelope<"run_failed", { errorCode: string; errorMessage: string }>
  | AgentRunEventEnvelope<"run_cancelled", Record<string, never>>;
