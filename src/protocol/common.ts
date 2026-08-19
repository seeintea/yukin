export type AppErrorCode =
  | "model_credential_missing"
  | "model_format_unsupported"
  | "model_authentication"
  | "model_rate_limited"
  | "model_invalid_request"
  | "model_upstream"
  | "model_timeout"
  | "model_transport"
  | "model_protocol"
  | "agent_step_limit"
  | "agent_tool_call_limit"
  | "tool_not_found"
  | "tool_invalid_arguments"
  | "tool_timeout"
  | "tool_approval_expired"
  | "tool_approval_invalid"
  | "tool_output_limit"
  | "tool_repeated_call"
  | "tool_execution"
  | "io"
  | "db"
  | "migrate"
  | "tauri"
  | "keyring"
  | "run_state"
  | "other";

export interface AppError {
  code: AppErrorCode;
  message: string;
}

export interface RecordMetadata {
  createdAt: string;
  updatedAt: string;
}
