import { invoke } from "@tauri-apps/api/core";

import type {
  Conversation,
  ConversationMessage,
  ConversationSnapshot,
  DeleteRequest,
  FindRequest,
  RenameRequest,
} from "#/protocol/conversation";

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

export function conversationMessageList(request: FindRequest): Promise<ConversationMessage[]> {
  return invoke("conversation_message_list", { request });
}

export function conversationRename(request: RenameRequest): Promise<Conversation> {
  return invoke("conversation_rename", { request });
}

export async function conversationDelete(request: DeleteRequest): Promise<void> {
  await invoke("conversation_delete", { request });
}
