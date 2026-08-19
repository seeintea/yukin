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
