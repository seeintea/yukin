import { ArrowUpIcon, LoaderCircleIcon } from "lucide-react";
import type { FormEvent, KeyboardEvent } from "react";
import { useState } from "react";

import { ModelSelector } from "#/components/model-selector";
import type { ModelSelection } from "#/components/model-selector";
import { Button } from "#/shadcn/button";
import { Textarea } from "#/shadcn/textarea";

export interface ChatInputValue {
  content: string;
  selection: ModelSelection;
}

interface ChatInputProps {
  isPending: boolean;
  onSubmit: (value: ChatInputValue) => void;
}

export function ChatInput({ isPending, onSubmit }: ChatInputProps) {
  const [content, setContent] = useState("");
  const [selection, setSelection] = useState<ModelSelection | null>(null);

  const submit = () => {
    const normalizedContent = content.trim();
    if (!normalizedContent || !selection || isPending) {
      return;
    }

    onSubmit({ content: normalizedContent, selection });
    setContent("");
  };

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    submit();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) {
      return;
    }

    event.preventDefault();
    submit();
  };

  return (
    <form className="w-full rounded-2xl border bg-card p-2 shadow-sm" onSubmit={handleSubmit}>
      <Textarea
        value={content}
        onChange={(event) => setContent(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入消息"
        disabled={isPending}
        className="max-h-48 min-h-24 resize-none border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent"
      />
      <div className="flex items-end justify-between gap-2 pt-2">
        <ModelSelector value={selection} onValueChange={setSelection} disabled={isPending} />
        <Button
          type="submit"
          size="icon"
          aria-label="发送消息"
          disabled={!content.trim() || !selection || isPending}
        >
          {isPending ? <LoaderCircleIcon className="animate-spin" /> : <ArrowUpIcon />}
        </Button>
      </div>
    </form>
  );
}
