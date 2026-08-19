import { invoke } from "@tauri-apps/api/core";

export type FrontendErrorKind =
  | "window"
  | "unhandled_rejection"
  | "react_uncaught"
  | "react_caught"
  | "react_recoverable";

export interface FrontendErrorReportRequest {
  kind: FrontendErrorKind;
  message: string;
  stack?: string;
  componentStack?: string;
  source?: string;
  line?: number;
  column?: number;
}

export async function frontendErrorReport(request: FrontendErrorReportRequest): Promise<void> {
  await invoke("diagnostics_frontend_error_report", { request });
}
