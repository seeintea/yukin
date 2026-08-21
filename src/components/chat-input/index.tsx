import { useMutation } from "@tanstack/react-query";
import {
  ArrowUpIcon,
  FileTextIcon,
  LoaderCircleIcon,
  PaperclipIcon,
  SquareIcon,
  XIcon,
} from "lucide-react";
import type { FormEvent, KeyboardEvent } from "react";
import { useState } from "react";

import { fileReferenceRelease, fileReferenceSelect } from "#/api/file";
import { ModelSelector } from "#/components/model-selector";
import type { ModelSelection } from "#/components/model-selector";
import { SkillSelector } from "#/components/skill-selector";
import type { FileReference } from "#/protocol/file";
import { Button } from "#/shadcn/button";
import { Textarea } from "#/shadcn/textarea";
import { showErrorToast } from "#/utils/toast";

export interface ChatInputValue {
  content: string;
  selection: ModelSelection;
  skillId: string | null;
  attachment: FileReference | null;
}

interface ChatInputProps {
  isPending: boolean;
  onSubmit: (value: ChatInputValue) => void;
  onCancel?: () => void;
}

export function ChatInput({ isPending, onSubmit, onCancel }: ChatInputProps) {
  const [content, setContent] = useState("");
  const [selection, setSelection] = useState<ModelSelection | null>(null);
  const [skillId, setSkillId] = useState<string | null>(null);
  const [attachment, setAttachment] = useState<FileReference | null>(null);
  const selectFileMutation = useMutation({
    mutationFn: fileReferenceSelect,
    onSuccess: (file) => {
      if (file) {
        setAttachment(file);
      }
    },
    onError: (error) => showErrorToast("选择文件失败", error),
  });
  const releaseFileMutation = useMutation({
    mutationFn: fileReferenceRelease,
    onError: (error) => showErrorToast("移除文件失败", error),
  });

  const submit = () => {
    const normalizedContent = content.trim();
    if (!normalizedContent || !selection || isPending) {
      return;
    }

    onSubmit({ content: normalizedContent, selection, skillId, attachment });
    setContent("");
    setAttachment(null);
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
    <form className="w-full rounded-2xl border bg-card p-2" onSubmit={handleSubmit}>
      <Textarea
        value={content}
        onChange={(event) => setContent(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入消息"
        disabled={isPending}
        className="max-h-48 min-h-24 resize-none border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent"
      />
      {attachment && (
        <div className="mx-1 mb-1 flex w-fit max-w-full items-center gap-2 rounded-lg border bg-muted/50 px-2.5 py-1.5 text-xs">
          <FileTextIcon className="size-4 shrink-0 text-muted-foreground" />
          <span className="truncate">{attachment.name}</span>
          <span className="shrink-0 text-muted-foreground">{formatFileSize(attachment.size)}</span>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label={`移除 ${attachment.name}`}
            disabled={isPending || releaseFileMutation.isPending}
            onClick={() => {
              const referenceId = attachment.referenceId;
              setAttachment(null);
              releaseFileMutation.mutate(referenceId);
            }}
          >
            <XIcon />
          </Button>
        </div>
      )}
      <div className="flex items-end justify-between gap-2 pt-2">
        <div className="flex flex-wrap items-center gap-1">
          {!attachment && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="选择文本文件"
              title="选择文本文件（最大 16 KiB）"
              disabled={isPending || selectFileMutation.isPending}
              onClick={() => selectFileMutation.mutate()}
            >
              {selectFileMutation.isPending ? (
                <LoaderCircleIcon className="animate-spin" />
              ) : (
                <PaperclipIcon />
              )}
            </Button>
          )}
          <ModelSelector value={selection} onValueChange={setSelection} disabled={isPending} />
          <SkillSelector value={skillId} onValueChange={setSkillId} disabled={isPending} />
        </div>
        <Button
          type={isPending && onCancel ? "button" : "submit"}
          size="icon"
          aria-label={isPending && onCancel ? "停止生成" : "发送消息"}
          disabled={isPending ? !onCancel : !content.trim() || !selection}
          onClick={isPending ? onCancel : undefined}
        >
          {isPending ? (
            onCancel ? (
              <SquareIcon className="fill-current" />
            ) : (
              <LoaderCircleIcon className="animate-spin" />
            )
          ) : (
            <ArrowUpIcon />
          )}
        </Button>
      </div>
    </form>
  );
}

function formatFileSize(size: number) {
  return size < 1024 ? `${size} B` : `${(size / 1024).toFixed(1)} KiB`;
}
