export type AppErrorCode = "io" | "db" | "migrate" | "tauri" | "keyring" | "other";

export interface AppError {
  code: AppErrorCode;
  message: string;
}

export interface RecordMetadata {
  createdAt: string;
  updatedAt: string;
}
