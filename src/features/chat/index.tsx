import { useState } from "react";

import { modelResponseStream } from "#/api/model-response";
import { ChatInput } from "#/components/chat-input";
import type { ChatInputValue } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import { toast } from "#/shadcn/toast";

interface Turn {
  userContent: string;
  assistantContent: string;
}

function getErrorMessage(error: unknown) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "请稍后重试";
}

export function Chat() {
  const [turn, setTurn] = useState<Turn | null>(null);
  const [isPending, setIsPending] = useState(false);

  const handleSubmit = async ({ content, selection }: ChatInputValue) => {
    setTurn({ userContent: content, assistantContent: "" });
    setIsPending(true);

    try {
      await modelResponseStream(
        {
          providerId: selection.providerId,
          modelId: selection.modelId,
          reasoningEffort: selection.reasoningEffort,
          content,
        },
        (event) => {
          if (event.event !== "output_delta") {
            return;
          }

          setTurn((current) =>
            current
              ? {
                  ...current,
                  assistantContent: current.assistantContent + event.data.content,
                }
              : current,
          );
        },
      );
    } catch (error) {
      toast.add({
        title: "消息发送失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    } finally {
      setIsPending(false);
    }
  };

  return (
    <main className="flex h-screen flex-col">
      <div className="flex-1 overflow-y-auto">
        {turn && (
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-10">
            <div className="ml-auto max-w-[80%] rounded-2xl bg-muted px-4 py-3 whitespace-pre-wrap">
              {turn.userContent}
            </div>
            <div className="max-w-none">
              {turn.assistantContent ? (
                <Markdown content={turn.assistantContent} isStreaming={isPending} />
              ) : isPending ? (
                "正在生成…"
              ) : null}
            </div>
          </div>
        )}
      </div>
      <div className="shrink-0 px-6 pt-4 pb-6">
        <div className="mx-auto w-full max-w-3xl">
          <ChatInput isPending={isPending} onSubmit={handleSubmit} />
        </div>
      </div>
    </main>
  );
}
