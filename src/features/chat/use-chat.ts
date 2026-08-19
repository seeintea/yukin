import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { conversationFind } from "#/api/conversation";
import { modelResponseStream } from "#/api/model-response";
import type { ChatInputValue } from "#/components/chat-input";
import type { ConversationMessage, ConversationSnapshot } from "#/protocol/conversation";
import { toast } from "#/shadcn/toast";

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

function createOptimisticMessage(
  role: ConversationMessage["role"],
  content: string,
  status: ConversationMessage["status"],
  sequence: number,
): ConversationMessage {
  const timestamp = new Date().toISOString();

  return {
    id: crypto.randomUUID(),
    role,
    content,
    status,
    sequence,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
}

export function useChat(conversationId: string) {
  const queryClient = useQueryClient();
  const queryKey = ["conversation", "find", conversationId] as const;
  const conversationQuery = useQuery({
    queryKey,
    queryFn: () => conversationFind({ id: conversationId }),
    staleTime: Infinity,
  });

  const sendMutation = useMutation({
    mutationFn: async ({ content, selection }: ChatInputValue) => {
      const current = queryClient.getQueryData<ConversationSnapshot>(queryKey);
      const lastMessage = current?.messages[current.messages.length - 1];
      const nextSequence = (lastMessage?.sequence ?? 0) + 1;
      const userMessage = createOptimisticMessage("user", content, "completed", nextSequence);
      const assistantMessage = createOptimisticMessage(
        "assistant",
        "",
        "streaming",
        nextSequence + 1,
      );

      queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
        snapshot
          ? { ...snapshot, messages: [...snapshot.messages, userMessage, assistantMessage] }
          : snapshot,
      );

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

          queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
            snapshot
              ? {
                  ...snapshot,
                  messages: snapshot.messages.map((message) =>
                    message.id === assistantMessage.id
                      ? { ...message, content: message.content + event.data.content }
                      : message,
                  ),
                }
              : snapshot,
          );
        },
      );
    },
    onError: (error) => {
      toast.add({
        title: "消息发送失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey });
    },
  });

  return {
    messages: conversationQuery.data?.messages ?? [],
    sendMessage: sendMutation.mutate,
    isPending: conversationQuery.isPending || sendMutation.isPending,
    isSending: sendMutation.isPending,
    isError: conversationQuery.isError,
  };
}
