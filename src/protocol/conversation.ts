export type MessageRole = "user" | "assistant" | "tool";
export type MessageStatus = "streaming" | "completed" | "failed" | "cancelled";

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationMessage {
  id: string;
  runId: string | null;
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

export interface FindRequest {
  id: string;
}
