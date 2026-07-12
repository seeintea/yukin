export async function* parseSse(
  stream: ReadableStream<Uint8Array>,
): AsyncGenerator<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let dataLines: string[] = [];

  while (true) {
    const { done, value } = await reader.read();

    buffer += done ? decoder.decode() : decoder.decode(value, { stream: true });

    let newlineIndex = buffer.indexOf("\n");

    while (newlineIndex !== -1) {
      let line = buffer.slice(0, newlineIndex);
      buffer = buffer.slice(newlineIndex + 1);

      if (line.endsWith("\r")) {
        line = line.slice(0, -1);
      }

      if (line === "") {
        if (dataLines.length > 0) {
          yield dataLines.join("\n");
          dataLines = [];
        }
      } else if (!line.startsWith(":")) {
        if (line === "data") {
          dataLines.push("");
        } else if (line.startsWith("data:")) {
          let data = line.slice(5);

          if (data.startsWith(" ")) {
            data = data.slice(1);
          }

          dataLines.push(data);
        }
      }

      newlineIndex = buffer.indexOf("\n");
    }

    if (done) break;
  }
}
