import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { conversationCurrent } from "#/api/conversation";
import { modelResponseStream } from "#/api/model-response";
import { ChatInput } from "#/components/chat-input";
import type { ChatInputValue } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import type { ConversationMessage, MessageRole, MessageStatus } from "#/protocol/conversation";
import { toast } from "#/shadcn/toast";

interface DisplayMessage {
  id: string;
  role: MessageRole;
  content: string;
  status: MessageStatus;
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
  const conversationQuery = useQuery({
    queryKey: ["conversation", "current"],
    queryFn: conversationCurrent,
    staleTime: Infinity,
  });
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [isPending, setIsPending] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isPending && conversationQuery.data) {
      setMessages(conversationQuery.data.messages.map(toDisplayMessage));
    }
  }, [conversationQuery.data, isPending]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: isPending ? "smooth" : "auto" });
  }, [isPending, messages]);

  const handleSubmit = async ({ content, selection }: ChatInputValue) => {
    const conversationId = conversationQuery.data?.conversation.id;
    if (!conversationId) {
      return;
    }

    const optimisticId = crypto.randomUUID();
    const assistantId = crypto.randomUUID();
    setMessages((current) => [
      ...current,
      { id: optimisticId, role: "user", content, status: "completed" },
      { id: assistantId, role: "assistant", content: "", status: "streaming" },
    ]);
    setIsPending(true);

    try {
      await modelResponseStream(
        {
          conversationId,
          providerId: selection.providerId,
          modelId: selection.modelId,
          reasoningEffort: selection.reasoningEffort,
          content,
        },
        (event) => {
          if (event.event !== "output_delta") {
            return;
          }

          setMessages((current) =>
            current.map((message) =>
              message.id === assistantId
                ? { ...message, content: message.content + event.data.content }
                : message,
            ),
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
      await conversationQuery.refetch();
      setIsPending(false);
    }
  };

  return (
    <main className="flex h-screen flex-col">
      <div className="flex-1 overflow-y-auto">
        {messages.length > 0 && (
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-10">
            {messages.map((message, index) =>
              message.role === "user" ? (
                <div
                  key={message.id}
                  className="ml-auto max-w-[80%] rounded-2xl bg-muted px-4 py-3 whitespace-pre-wrap"
                >
                  {message.content}
                </div>
              ) : (
                <div key={message.id} className="max-w-none">
                  {message.content ? (
                    <Markdown
                      content={message.content}
                      isStreaming={isPending && index === messages.length - 1}
                    />
                  ) : message.status === "streaming" ? (
                    "正在生成…"
                  ) : message.status === "failed" ? (
                    <span className="text-muted-foreground">生成失败</span>
                  ) : null}
                </div>
              ),
            )}
            <div ref={bottomRef} />
          </div>
        )}
      </div>
      <div className="shrink-0 px-6 pt-4 pb-6">
        <div className="mx-auto w-full max-w-3xl">
          <ChatInput
            isPending={isPending || conversationQuery.isPending || conversationQuery.isError}
            onSubmit={handleSubmit}
          />
        </div>
      </div>
    </main>
  );
}

function toDisplayMessage(message: ConversationMessage): DisplayMessage {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    status: message.status,
  };
}
