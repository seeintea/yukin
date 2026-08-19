import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { conversationCurrent } from "#/api/conversation";
import { Chat } from "#/features/chat";

export const Route = createFileRoute("/chat")({
  component: ChatRoute,
});

function ChatRoute() {
  const conversationQuery = useQuery({
    queryKey: ["conversation", "current"],
    queryFn: conversationCurrent,
    staleTime: Infinity,
  });

  return conversationQuery.data ? <Chat conversationId={conversationQuery.data.id} /> : null;
}
