export async function streamDeepSeek(
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
      thinking: { type: "enabled" },
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

  const reader = response.body.getReader();
  const decoder = new TextDecoder();

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    const chunk = decoder.decode(value, { stream: true });
    console.log("raw chunk:", JSON.stringify(chunk));
  }
}
