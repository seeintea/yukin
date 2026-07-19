import type { ProtocolErrorCode } from "../protocol/types";

export type DeepSeekErrorCode =
  | "REQUEST_FAILED"
  | "HTTP_ERROR"
  | "UNSUPPORTED_FORMAT"
  | ProtocolErrorCode;

export class DeepSeekError extends Error {
  constructor(
    readonly code: DeepSeekErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "DeepSeekError";
  }
}
