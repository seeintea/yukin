import { invoke } from "@tauri-apps/api/core";

import type { ConversationSnapshot } from "#/protocol/conversation";

export function conversationCurrent(): Promise<ConversationSnapshot> {
  return invoke("conversation_current");
}
