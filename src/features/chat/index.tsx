import { useMutation } from "@tanstack/react-query";
import { ExternalLinkIcon, FileSearchIcon, FileTextIcon, FolderIcon } from "lucide-react";
import { useEffect, useRef } from "react";

import { directoryEntryOpen, directoryEntryReveal } from "#/api/file";
import { ChatInput } from "#/components/chat-input";
import { Markdown } from "#/components/markdown";
import type { ActiveToolCall } from "#/protocol/agent-run";
import type { Conversation } from "#/protocol/conversation";
import { Button } from "#/shadcn/button";
import { Card, CardContent, CardHeader, CardTitle } from "#/shadcn/card";
import { SidebarInset, SidebarProvider } from "#/shadcn/sidebar";
import { showErrorToast } from "#/utils/toast";

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
        <CardTitle>
          {{
            read_selected_text_file: "读取文本文件",
            list_selected_directory: "列出目录内容",
            search_selected_directory: "搜索目录",
            get_directory_entry_metadata: "读取文件元信息",
            open_directory_entry: "打开文件",
            reveal_directory_entry: "在系统中显示",
          }[toolCall.name] ?? toolCall.name}
        </CardTitle>
        <span className="text-xs text-muted-foreground">{status}</span>
      </CardHeader>
      <CardContent className="space-y-2 text-xs">
        {toolCall.name === "read_selected_text_file" ? (
          <FileReadResult value={toolCall.result} />
        ) : toolCall.name === "list_selected_directory" ? (
          <DirectoryListResult value={toolCall.result} />
        ) : toolCall.name === "search_selected_directory" ? (
          <DirectorySearchResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "get_directory_entry_metadata" ? (
          <DirectoryEntryMetadataResult value={toolCall.result} />
        ) : toolCall.name === "open_directory_entry" ||
          toolCall.name === "reveal_directory_entry" ? (
          <DirectoryEntryActionResult
            argumentsValue={toolCall.arguments}
            value={toolCall.result}
            action={toolCall.name === "open_directory_entry" ? "open" : "reveal"}
          />
        ) : (
          <>
            <ToolCallValue label="参数" value={toolCall.arguments} />
            {toolCall.result !== null && <ToolCallValue label="结果" value={toolCall.result} />}
          </>
        )}
        {toolCall.errorMessage && (
          <p className="text-destructive">
            {formatToolError(toolCall.errorCode, toolCall.errorMessage)}
          </p>
        )}
        {toolCall.status === "waiting_approval" && (
          <div className="flex justify-end gap-2 pt-2">
            <Button size="sm" variant="outline" disabled={isDeciding} onClick={onReject}>
              拒绝
            </Button>
            <Button size="sm" disabled={isDeciding} onClick={onAllow}>
              {isDeciding
                ? "提交中…"
                : toolCall.name === "open_directory_entry"
                  ? "允许打开"
                  : toolCall.name === "reveal_directory_entry"
                    ? "允许显示"
                    : "允许"}
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function DirectorySearchResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const searchArguments =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as { query?: unknown; kind?: unknown })
      : {};
  if (!value || typeof value !== "object") {
    return (
      <p className="text-muted-foreground">
        正在已授权目录中搜索“{String(searchArguments.query ?? "")}”…
      </p>
    );
  }
  const result = value as {
    directoryName?: unknown;
    query?: unknown;
    kind?: unknown;
    entries?: unknown;
    truncated?: unknown;
  };
  const entries = Array.isArray(result.entries) ? result.entries : [];
  const kind =
    { file: "文件", directory: "目录", any: "文件和目录" }[String(result.kind)] ?? "项目";
  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? result.directoryName : "已授权目录"} · 搜索“
        {String(result.query ?? searchArguments.query ?? "")}” · {entries.length} 个{kind}
        {result.truncated === true ? "（结果已截断）" : ""}
      </p>
      <div className="grid gap-1">
        {entries.map((entry, index) => {
          const item = entry as {
            relativePath?: unknown;
            kind?: unknown;
            targetReferenceId?: unknown;
          };
          return (
            <DirectoryEntryRow
              key={`${String(item.relativePath)}-${index}`}
              kind={String(item.kind ?? "other")}
              label={String(item.relativePath ?? "")}
              targetReferenceId={
                typeof item.targetReferenceId === "string" ? item.targetReferenceId : null
              }
            />
          );
        })}
      </div>
    </div>
  );
}

function DirectoryListResult({ value }: { value: unknown }) {
  if (!value || typeof value !== "object") {
    return <p className="text-muted-foreground">正在列出已授权目录…</p>;
  }
  const result = value as {
    directoryName?: unknown;
    entries?: unknown;
    truncated?: unknown;
  };
  const entries = Array.isArray(result.entries) ? result.entries : [];
  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? result.directoryName : "已授权目录"} ·{" "}
        {entries.length} 项{result.truncated === true ? "（结果已截断）" : ""}
      </p>
      <div className="grid gap-1">
        {entries.map((entry, index) => {
          const item = entry as {
            name?: unknown;
            kind?: unknown;
            targetReferenceId?: unknown;
          };
          return (
            <DirectoryEntryRow
              key={`${String(item.name)}-${index}`}
              kind={String(item.kind ?? "other")}
              label={String(item.name ?? "")}
              targetReferenceId={
                typeof item.targetReferenceId === "string" ? item.targetReferenceId : null
              }
            />
          );
        })}
      </div>
    </div>
  );
}

function DirectoryEntryRow({
  kind,
  label,
  targetReferenceId,
}: {
  kind: string;
  label: string;
  targetReferenceId: string | null;
}) {
  const actionMutation = useMutation({
    mutationFn: async (action: "open" | "reveal") => {
      if (!targetReferenceId) {
        return;
      }
      if (action === "open") {
        await directoryEntryOpen(targetReferenceId);
      } else {
        await directoryEntryReveal(targetReferenceId);
      }
    },
    onError: (error) => showErrorToast("文件操作失败", error),
  });

  return (
    <div className="flex min-w-0 items-center gap-2">
      <span className="text-muted-foreground">{kind}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {targetReferenceId && (
        <div className="flex shrink-0 gap-1">
          <Button
            size="icon-xs"
            variant="ghost"
            title="使用默认应用打开"
            aria-label={`打开 ${label}`}
            disabled={actionMutation.isPending}
            onClick={() => actionMutation.mutate("open")}
          >
            <ExternalLinkIcon />
          </Button>
          <Button
            size="icon-xs"
            variant="ghost"
            title="在系统文件管理器中显示"
            aria-label={`在系统中显示 ${label}`}
            disabled={actionMutation.isPending}
            onClick={() => actionMutation.mutate("reveal")}
          >
            <FileSearchIcon />
          </Button>
        </div>
      )}
    </div>
  );
}

function DirectoryEntryMetadataResult({ value }: { value: unknown }) {
  if (!value || typeof value !== "object") {
    return <p className="text-muted-foreground">正在读取文件元信息…</p>;
  }
  const result = value as {
    relativePath?: unknown;
    kind?: unknown;
    size?: unknown;
    modifiedAt?: unknown;
    extension?: unknown;
  };
  return (
    <div className="space-y-1">
      <p className="font-medium">{String(result.relativePath ?? "已授权条目")}</p>
      <p className="text-muted-foreground">
        {String(result.kind ?? "other")}
        {typeof result.extension === "string" ? ` · .${result.extension}` : ""}
        {typeof result.size === "number" ? ` · ${formatFileSize(result.size)}` : ""}
        {typeof result.modifiedAt === "string"
          ? ` · 修改于 ${formatDateTime(result.modifiedAt)}`
          : ""}
      </p>
    </div>
  );
}

function DirectoryEntryActionResult({
  action,
  argumentsValue,
  value,
}: {
  action: "open" | "reveal";
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as { relativePath?: unknown })
      : {};
  const result = value && typeof value === "object" ? (value as { relativePath?: unknown }) : {};
  return (
    <p>
      {String(result.relativePath ?? argumentsObject.relativePath ?? "已授权条目")} ·{" "}
      {value ? (action === "open" ? "已打开" : "已在系统中显示") : "等待执行"}
    </p>
  );
}

function FileReadResult({ value }: { value: unknown }) {
  if (!value || typeof value !== "object") {
    return <p className="text-muted-foreground">正在读取已授权附件…</p>;
  }
  const result = value as { fileName?: unknown; size?: unknown };
  return (
    <p>
      {typeof result.fileName === "string" ? result.fileName : "已授权附件"}
      {typeof result.size === "number" ? ` · ${formatFileSize(result.size)}` : ""}
    </p>
  );
}

function formatFileSize(size: number) {
  return size < 1024 ? `${size} B` : `${(size / 1024).toFixed(1)} KiB`;
}

function formatDateTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function formatToolError(code: string | null, fallback: string) {
  return (
    {
      tool_timeout: "操作超时，请缩小目录范围后重试。",
      file_io: "无法访问该条目，请检查文件是否仍存在以及当前账户是否有权限。",
      directory_not_found: "授权目录已不存在或当前账户无权访问。",
      file_symlink_unsupported: "目标已变为符号链接，出于安全原因已停止操作。",
      file_changed: "授权目录在操作前发生变化，请重新选择目录。",
      directory_entry_reference_invalid: "文件条目引用已失效，请重新列出或搜索目录。",
      file_system_action: "系统无法完成该文件操作。",
    }[code ?? ""] ?? fallback
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
