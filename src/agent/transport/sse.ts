export async function* parseSse(
  stream: ReadableStream<Uint8Array>,
): AsyncGenerator<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { done, value } = await reader.read();

    if (done) break;

    buffer += decoder.decode(value, { stream: true });

    /**
     * 接收到的原始数据
     *
     * data: {"id":"3f4410e3-64fc-4adc-a15c-aa90ffc5af83","object":"chat.completion.chunk","created":1783825304,"model":"deepseek-v4-pro","system_fingerprint":"fp_9954b31ca7_prod0820_fp8_kvcache_20260402","choices":[{"index":0,"delta":{"content":null,"reasoning_content":":"},"logprobs":null,"finish_reason":null}]}
     *
     *
     * data: {"id":"3f4410e3-64fc-4adc-a15c-aa90ffc5af83","object":"chat.completion.chunk","created":1783825304,"model":"deepseek-v4-pro","system_fingerprint":"fp_9954b31ca7_prod0820_fp8_kvcache_20260402","choices":[{"index":0,"delta":{"content":null,"reasoning_content":"1"},"logprobs":null,"finish_reason":null}]}
     */
    const blocks = buffer.split("\n\n");
    buffer = blocks.pop() ?? "";

    for (const block of blocks) {
      // 移除 fetch 返回的 `data: `
      // 之后的数据可以被完美 JSON.parse
      if (block.startsWith("data: ")) {
        yield block.slice(6);
      }
    }
  }
}
