import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { conversationCreate } from "#/api/conversation";
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

  const conversations = conversationsQuery.data ?? [];
  const requestedConversation = conversations.find(
    (conversation) => conversation.id === search.conversationId,
  );
  const conversationId = requestedConversation?.id ?? currentConversationQuery.data?.id;

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
    />
  );
}
