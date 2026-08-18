import { Streamdown } from "streamdown";

interface MarkdownProps {
  content: string;
  isStreaming: boolean;
}

export function Markdown({ content, isStreaming }: MarkdownProps) {
  return (
    <Streamdown isAnimating={isStreaming} mode={isStreaming ? "streaming" : "static"}>
      {content}
    </Streamdown>
  );
}
