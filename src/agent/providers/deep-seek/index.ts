import { BaseProvider } from "../provider";
import { OpenAIChatProtocol, ProtocolError } from "../protocol/open-ai";
import type {
  ProviderChatInput,
  ProviderConnection,
  ProviderEvent,
} from "../types";
import { DeepSeekError } from "./error";
import type { DeepSeekProviderOptions } from "./types";
import { DEFAULT_DEEP_SEEK_OPTIONS } from "./variable";

export class DeepSeekProvider extends BaseProvider<DeepSeekProviderOptions> {
  private readonly protocol = new OpenAIChatProtocol();

  constructor(
    connection: ProviderConnection,
    options: DeepSeekProviderOptions = DEFAULT_DEEP_SEEK_OPTIONS,
  ) {
    super(connection, options);
  }

  async *chat(input: ProviderChatInput): AsyncIterable<ProviderEvent> {
    if (this.format !== "open-ai") {
      throw new DeepSeekError(
        "UNSUPPORTED_FORMAT",
        "DeepSeekProvider 暂时只支持 OpenAI 兼容格式",
      );
    }

    let response: Response;

    try {
      response = await fetch(this.connection.baseUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${this.connection.apiKey}`,
        },
        body: JSON.stringify(
          this.protocol.createRequestBody(input, {
            model: this.model,
            stream: this.stream,
            extensions: {
              thinking: {
                type: this.options.thinking ? "enabled" : "disabled",
              },
              reasoning_effort: this.options.reasoningEffort,
            },
          }),
        ),
        signal: input.signal,
      });
    } catch {
      throw new DeepSeekError("REQUEST_FAILED", "DeepSeek 请求发送失败");
    }

    if (!response.ok) {
      const message = await response.text();
      throw new DeepSeekError(
        "HTTP_ERROR",
        `DeepSeek 请求失败：${response.status} ${message}`,
      );
    }

    try {
      yield* this.protocol.readResponse(response, this.stream);
    } catch (error) {
      if (error instanceof ProtocolError) {
        throw new DeepSeekError(error.code, error.message);
      }

      if (error instanceof DeepSeekError) throw error;
      throw new DeepSeekError("RESPONSE_READ_FAILED", "DeepSeek 响应读取失败");
    }
  }
}

export { DeepSeekError } from "./error";
