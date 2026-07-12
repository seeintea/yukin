import { parseSse } from "../transport/sse";

export async function* streamDeepSeek(
  url: string,
  key: string,
  messages: { role: string; content: string }[],
) {
  const response = await fetch(`${url}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${key}`,
    },
    body: JSON.stringify({
      messages,
      model: "deepseek-v4-pro",
      // thinking: { type: "enabled" },
      thinking: { type: "disabled" }, // 暂时关闭思考模式
      reasoning_effort: "high",
      stream: true,
    }),
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(`DeepSeek 请求失败：${response.status} ${message}`);
  }

  if (!response.body) {
    throw new Error("DeepSeek 没有返回响应流");
  }

  for await (const data of parseSse(response.body)) {
    if (data === "[DONE]") break;

    const payload = JSON.parse(data);

    const content = payload.choices[0]?.delta?.content;

    if (content) {
      yield content;
    }
  }
}
