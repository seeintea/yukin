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
  attachments: MessageAttachment[];
  directoryScopes: MessageDirectoryScope[];
  status: MessageStatus;
  sequence: number;
  createdAt: string;
  updatedAt: string;
}

export interface MessageAttachment {
  name: string;
  size: number;
}

export interface MessageDirectoryScope {
  name: string;
}

export interface ConversationSnapshot {
  conversation: Conversation;
  messages: ConversationMessage[];
}

export interface FindRequest {
  id: string;
}

export interface RenameRequest {
  id: string;
  title: string;
}

export interface DeleteRequest {
  id: string;
}
