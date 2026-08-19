import { useEffect, useRef } from "react";

import { ChatInput } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import type { Conversation } from "#/protocol/conversation";
import { SidebarInset, SidebarProvider } from "#/shadcn/sidebar";

import { ConversationSidebar } from "./conversation-sidebar";
import { useChat } from "./use-chat";

interface ChatProps {
  conversationId: string;
  conversations: Conversation[];
  isCreatingConversation: boolean;
  onCreateConversation: () => void;
  onSelectConversation: (conversationId: string) => void;
}

export function Chat({
  conversationId,
  conversations,
  isCreatingConversation,
  onCreateConversation,
  onSelectConversation,
}: ChatProps) {
  return (
    <SidebarProvider className="h-svh min-h-0 overflow-hidden">
      <ConversationSidebar
        conversations={conversations}
        selectedConversationId={conversationId}
        isCreating={isCreatingConversation}
        onCreate={onCreateConversation}
        onSelect={onSelectConversation}
      />
      <SidebarInset className="h-svh min-w-0 overflow-hidden">
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
    isPending,
    isSending,
    isCancelling,
    isError,
  } = useChat(conversationId);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
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
