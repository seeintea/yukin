import type { ProviderChatInput, ProviderEvent } from "../types";

export interface ProtocolRequestOptions {
  model: string;
  stream: boolean;
  extensions?: Record<string, unknown>;
}

export interface ChatProtocol {
  createRequestBody(
    input: ProviderChatInput,
    options: ProtocolRequestOptions,
  ): Record<string, unknown>;

  readResponse(
    response: Response,
    stream: boolean,
  ): AsyncIterable<ProviderEvent>;
}

export type ProtocolErrorCode =
  | "EMPTY_RESPONSE_BODY"
  | "RESPONSE_READ_FAILED"
  | "INVALID_RESPONSE_DATA"
  | "API_ERROR"
  | "INCOMPLETE_STREAM"
  | "MISSING_FINISH_REASON";
