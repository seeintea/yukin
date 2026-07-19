import type {
  ProviderChatInput,
  ProviderConnection,
  ProviderEvent,
  ProviderOptions,
} from "./types";

/**
 * Agent Runtime 与具体模型供应商之间的统一边界。
 *
 * TOptions 让具体 Provider 在 model/stream 之外增加自己的配置：
 *
 * DeepSeek 可以增加 thinking、reasoningEffort，而不会污染其他 Provider。
 */
export abstract class BaseProvider<
  TOptions extends ProviderOptions = ProviderOptions,
> {
  protected constructor(
    protected readonly connection: Readonly<ProviderConnection>,
    readonly options: Readonly<TOptions>,
  ) {}

  get format() {
    return this.connection.format;
  }

  get model() {
    return this.options.model;
  }

  get stream() {
    return this.options.stream;
  }

  /**
   * 无论底层采用流式还是非流式 HTTP 响应，Runtime 都通过同一个
   * AsyncIterable 消费统一 ProviderEvent。
   */
  abstract chat(input: ProviderChatInput): AsyncIterable<ProviderEvent>;
}
