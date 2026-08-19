import { queryOptions } from "@tanstack/react-query";

import { conversationCurrent, conversationFind, conversationList } from "#/api/conversation";

export const conversationKeys = {
  current: ["conversation", "current"] as const,
  list: ["conversation", "list"] as const,
  find: (conversationId: string) => ["conversation", "find", conversationId] as const,
};

export const currentConversationQueryOptions = queryOptions({
  queryKey: conversationKeys.current,
  queryFn: conversationCurrent,
  staleTime: Infinity,
});

export const conversationListQueryOptions = queryOptions({
  queryKey: conversationKeys.list,
  queryFn: conversationList,
  staleTime: Infinity,
});

export function conversationQueryOptions(conversationId: string) {
  return queryOptions({
    queryKey: conversationKeys.find(conversationId),
    queryFn: () => conversationFind({ id: conversationId }),
    staleTime: Infinity,
  });
}
