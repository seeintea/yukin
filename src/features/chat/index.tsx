import { FileTextIcon, FolderIcon } from "lucide-react";
import { useEffect, useRef } from "react";

import { ChatInput } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import type { Conversation } from "#/protocol/conversation";
import { SidebarInset, SidebarProvider } from "#/shadcn/sidebar";

import { ConversationSidebar } from "./conversation-sidebar";
import { ToolCallCard } from "./tool-call-card";
import { formatFileSize } from "./tool-call-results";
import { useChat } from "./use-chat";

interface ChatProps {
  conversationId: string;
  conversations: Conversation[];
  isCreatingConversation: boolean;
  onCreateConversation: () => void;
  onSelectConversation: (conversationId: string) => void;
  onRenameConversation: (conversationId: string, title: string) => Promise<void>;
  onDeleteConversation: (conversationId: string) => Promise<void>;
  renamingConversationId: string | null;
  deletingConversationId: string | null;
}

export function Chat({
  conversationId,
  conversations,
  isCreatingConversation,
  onCreateConversation,
  onSelectConversation,
  onRenameConversation,
  onDeleteConversation,
  renamingConversationId,
  deletingConversationId,
}: ChatProps) {
  return (
    <SidebarProvider className="h-full min-h-0 overflow-hidden">
      <ConversationSidebar
        conversations={conversations}
        selectedConversationId={conversationId}
        isCreating={isCreatingConversation}
        onCreate={onCreateConversation}
        onSelect={onSelectConversation}
        onRename={onRenameConversation}
        onDelete={onDeleteConversation}
        renamingConversationId={renamingConversationId}
        deletingConversationId={deletingConversationId}
      />
      <SidebarInset className="h-full min-w-0 overflow-hidden">
        <ChatConversation key={conversationId} conversationId={conversationId} />
      </SidebarInset>
    </SidebarProvider>
  );
}

function ChatConversation({ conversationId }: Pick<ChatProps, "conversationId">) {
  const {
    messages,
    sendMessage,
    cancelRun,
    canCancel,
    phase,
    toolCalls,
    decideToolCall,
    decidingToolCallId,
    isPending,
    isSending,
    isCancelling,
    isError,
  } = useChat(conversationId);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (messages.length === 0) return;
    bottomRef.current?.scrollIntoView({ behavior: isSending ? "smooth" : "auto" });
  }, [isSending, messages]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex-1 overflow-y-auto">
        {messages.length > 0 && (
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-10">
            {messages.map((message, index) =>
              message.role === "user" ? (
                <div
                  key={message.id}
                  className="ml-auto max-w-[80%] rounded-2xl bg-muted px-4 py-3 whitespace-pre-wrap"
                >
                  {message.content}
                  {message.attachments.map((attachment) => (
                    <div
                      key={attachment.name}
                      className="mt-2 flex items-center gap-2 rounded-lg border bg-background/60 px-2.5 py-1.5 text-xs"
                    >
                      <FileTextIcon className="size-4 shrink-0 text-muted-foreground" />
                      <span className="min-w-0 truncate">{attachment.name}</span>
                      <span className="shrink-0 text-muted-foreground">
                        {formatFileSize(attachment.size)}
                      </span>
                    </div>
                  ))}
                  {message.directoryScopes.map((scope) => (
                    <div
                      key={scope.name}
                      className="mt-2 flex items-center gap-2 rounded-lg border bg-background/60 px-2.5 py-1.5 text-xs"
                    >
                      <FolderIcon className="size-4 shrink-0 text-muted-foreground" />
                      <span className="min-w-0 truncate">{scope.name}</span>
                      <span className="shrink-0 text-muted-foreground">目录范围</span>
                    </div>
                  ))}
                </div>
              ) : (
                <div key={message.id} className="max-w-none">
                  {message.content ? (
                    <Markdown
                      content={message.content}
                      isStreaming={isSending && index === messages.length - 1}
                    />
                  ) : message.status === "streaming" ? (
                    phase === "thinking" ? (
                      "正在思考…"
                    ) : (
                      "正在生成…"
                    )
                  ) : message.status === "failed" ? (
                    <span className="text-muted-foreground">生成失败</span>
                  ) : message.status === "cancelled" ? (
                    <span className="text-muted-foreground">已停止生成</span>
                  ) : null}
                </div>
              ),
            )}
            {toolCalls.map((toolCall) => (
              <ToolCallCard
                key={toolCall.id}
                toolCall={toolCall}
                isDeciding={decidingToolCallId === toolCall.id}
                onAllow={() => decideToolCall(toolCall, "allow")}
                onReject={() => decideToolCall(toolCall, "reject")}
              />
            ))}
            <div ref={bottomRef} />
          </div>
        )}
      </div>
      <div className="shrink-0 px-6 pt-4 pb-6">
        <div className="mx-auto w-full max-w-3xl">
          <ChatInput
            isPending={isPending || isError}
            onSubmit={sendMessage}
            onCancel={isSending && canCancel && !isCancelling ? cancelRun : undefined}
          />
        </div>
      </div>
    </div>
  );
}
