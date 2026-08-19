import { invoke } from "@tauri-apps/api/core";

import type { Conversation, ConversationSnapshot, FindRequest } from "#/protocol/conversation";

export function conversationCurrent(): Promise<Conversation> {
  return invoke("conversation_current");
}

export function conversationFind(request: FindRequest): Promise<ConversationSnapshot> {
  return invoke("conversation_find", { request });
}

export function conversationList(): Promise<Conversation[]> {
  return invoke("conversation_list");
}

export function conversationCreate(): Promise<Conversation> {
  return invoke("conversation_create");
}
