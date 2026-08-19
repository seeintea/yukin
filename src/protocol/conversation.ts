export type MessageRole = "user" | "assistant";
export type MessageStatus = "streaming" | "completed" | "failed";

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationMessage {
  id: string;
  role: MessageRole;
  content: string;
  status: MessageStatus;
  sequence: number;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationSnapshot {
  conversation: Conversation;
  messages: ConversationMessage[];
}
