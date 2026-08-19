import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { conversationCreate, conversationDelete, conversationRename } from "#/api/conversation";
import { Chat } from "#/features/chat";
import {
  conversationKeys,
  conversationListQueryOptions,
  currentConversationQueryOptions,
} from "#/features/chat/queries";
import type { Conversation } from "#/protocol/conversation";
import { toast } from "#/shadcn/toast";

interface ChatSearch {
  conversationId?: string;
}

export const Route = createFileRoute("/chat")({
  component: ChatRoute,
  validateSearch: (search): ChatSearch => ({
    conversationId:
      typeof search.conversationId === "string" && search.conversationId
        ? search.conversationId
        : undefined,
  }),
});

function ChatRoute() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  const queryClient = useQueryClient();
  const currentConversationQuery = useQuery(currentConversationQueryOptions);
  const conversationsQuery = useQuery({
    ...conversationListQueryOptions,
    enabled: currentConversationQuery.isSuccess,
  });
  const createMutation = useMutation({
    mutationFn: conversationCreate,
    onSuccess: async (conversation) => {
      queryClient.setQueryData<Conversation[]>(conversationKeys.list, (conversations = []) => [
        conversation,
        ...conversations.filter((item) => item.id !== conversation.id),
      ]);
      queryClient.setQueryData(conversationKeys.current, conversation);
      await navigate({ search: { conversationId: conversation.id } });
    },
    onError: () => {
      toast.add({
        title: "新建对话失败",
        description: "请稍后重试",
        type: "error",
        priority: "high",
      });
    },
  });
  const renameMutation = useMutation({
    mutationFn: conversationRename,
    onSuccess: (conversation) => {
      queryClient.setQueryData<Conversation[]>(conversationKeys.list, (conversations = []) => [
        conversation,
        ...conversations.filter((item) => item.id !== conversation.id),
      ]);
      queryClient.setQueryData<Conversation | undefined>(conversationKeys.current, (current) =>
        current?.id === conversation.id ? conversation : current,
      );
      queryClient.setQueryData(conversationKeys.find(conversation.id), (snapshot) =>
        snapshot ? { ...snapshot, conversation } : snapshot,
      );
      toast.add({ title: "会话已重命名", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "会话重命名失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });

  const conversations = conversationsQuery.data ?? [];
  const requestedConversation = conversations.find(
    (conversation) => conversation.id === search.conversationId,
  );
  const conversationId = requestedConversation?.id ?? currentConversationQuery.data?.id;

  const deleteMutation = useMutation({
    mutationFn: conversationDelete,
    onSuccess: async (_, request) => {
      const cachedConversations =
        queryClient.getQueryData<Conversation[]>(conversationKeys.list) ?? conversations;
      const remaining = cachedConversations.filter((item) => item.id !== request.id);
      const cachedCurrent = queryClient.getQueryData<Conversation>(conversationKeys.current);

      queryClient.setQueryData(conversationKeys.list, remaining);
      queryClient.removeQueries({ queryKey: conversationKeys.find(request.id) });

      if (remaining.length === 0) {
        await createMutation.mutateAsync();
      } else {
        const nextConversation =
          remaining.find((conversation) => conversation.id === conversationId) ?? remaining[0];
        if (cachedCurrent?.id === request.id) {
          queryClient.setQueryData(conversationKeys.current, nextConversation);
        }
        if (request.id === conversationId) {
          await navigate({ search: { conversationId: nextConversation.id } });
        }
      }
      toast.add({ title: "会话已删除", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "会话删除失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });

  if (!conversationId) {
    return null;
  }

  const handleSelectConversation = (selectedConversationId: string) => {
    void navigate({ search: { conversationId: selectedConversationId } });
  };

  return (
    <Chat
      conversationId={conversationId}
      conversations={conversations}
      isCreatingConversation={createMutation.isPending}
      onCreateConversation={() => createMutation.mutate()}
      onSelectConversation={handleSelectConversation}
      onRenameConversation={async (id, title) => {
        await renameMutation.mutateAsync({ id, title });
      }}
      onDeleteConversation={async (id) => {
        await deleteMutation.mutateAsync({ id });
      }}
      renamingConversationId={
        renameMutation.isPending ? (renameMutation.variables?.id ?? null) : null
      }
      deletingConversationId={
        deleteMutation.isPending ? (deleteMutation.variables?.id ?? null) : null
      }
    />
  );
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
