import { useEffect, useRef } from "react";

import { ChatInput } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import type { ActiveToolCall } from "#/protocol/agent-run";
import type { Conversation } from "#/protocol/conversation";
import { Button } from "#/shadcn/button";
import { Card, CardContent, CardHeader, CardTitle } from "#/shadcn/card";
import { SidebarInset, SidebarProvider } from "#/shadcn/sidebar";

import { ConversationSidebar } from "./conversation-sidebar";
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
    <SidebarProvider className="h-svh min-h-0 overflow-hidden">
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

function ToolCallCard({
  toolCall,
  isDeciding,
  onAllow,
  onReject,
}: {
  toolCall: ActiveToolCall;
  isDeciding: boolean;
  onAllow: () => void;
  onReject: () => void;
}) {
  const status = {
    requested: "等待执行",
    waiting_approval: "等待批准",
    running: "执行中…",
    completed: "已完成",
    failed: "执行失败",
    rejected: "已拒绝",
    cancelled: "已取消",
  }[toolCall.status];

  return (
    <Card size="sm" className="max-w-xl bg-muted/30">
      <CardHeader className="grid-cols-[1fr_auto]">
        <CardTitle>{toolCall.name}</CardTitle>
        <span className="text-xs text-muted-foreground">{status}</span>
      </CardHeader>
      <CardContent className="space-y-2 text-xs">
        <ToolCallValue label="参数" value={toolCall.arguments} />
        {toolCall.result !== null && <ToolCallValue label="结果" value={toolCall.result} />}
        {toolCall.errorMessage && <p className="text-destructive">{toolCall.errorMessage}</p>}
        {toolCall.status === "waiting_approval" && (
          <div className="flex justify-end gap-2 pt-2">
            <Button size="sm" variant="outline" disabled={isDeciding} onClick={onReject}>
              拒绝
            </Button>
            <Button size="sm" disabled={isDeciding} onClick={onAllow}>
              {isDeciding ? "提交中…" : "允许"}
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function ToolCallValue({ label, value }: { label: string; value: unknown }) {
  return (
    <div>
      <div className="mb-1 text-muted-foreground">{label}</div>
      <pre className="overflow-x-auto whitespace-pre-wrap">{JSON.stringify(value, null, 2)}</pre>
    </div>
  );
}
