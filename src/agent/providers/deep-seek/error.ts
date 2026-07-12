export type DeepSeekErrorCode =
  | "REQUEST_FAILED"
  | "HTTP_ERROR"
  | "EMPTY_RESPONSE_BODY"
  | "STREAM_READ_FAILED"
  | "INVALID_STREAM_DATA"
  | "API_ERROR"
  | "INCOMPLETE_STREAM"
  | "MISSING_FINISH_REASON";

export class DeepSeekError extends Error {
  constructor(
    public readonly code: DeepSeekErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "DeepSeekError";
  }
}
