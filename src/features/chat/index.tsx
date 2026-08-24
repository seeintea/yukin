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
            create_text_file_in_selected_directory: "创建文本文件",
            create_directory_in_selected_directory: "创建目录",
            copy_directory_entry: "复制文件或目录",
            move_directory_entry: "移动或重命名",
            trash_directory_entry: "移入系统回收站",
            batch_move_directory_entries: "批量移动或重命名",
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
        ) : toolCall.name === "create_text_file_in_selected_directory" ? (
          <CreateTextFileResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "create_directory_in_selected_directory" ? (
          <CreateDirectoryResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "copy_directory_entry" ? (
          <CopyDirectoryEntryResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "move_directory_entry" ? (
          <MoveDirectoryEntryResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "trash_directory_entry" ? (
          <TrashDirectoryEntryResult argumentsValue={toolCall.arguments} value={toolCall.result} />
        ) : toolCall.name === "batch_move_directory_entries" ? (
          <BatchMoveDirectoryEntriesResult
            argumentsValue={toolCall.arguments}
            value={toolCall.result}
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
                    : toolCall.name === "create_text_file_in_selected_directory"
                      ? "允许创建"
                      : toolCall.name === "create_directory_in_selected_directory"
                        ? "允许创建"
                        : toolCall.name === "copy_directory_entry"
                          ? "允许复制"
                          : toolCall.name === "move_directory_entry"
                            ? "允许移动"
                            : toolCall.name === "trash_directory_entry"
                              ? "允许移入回收站"
                              : toolCall.name === "batch_move_directory_entries"
                                ? "允许批量移动"
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

function CreateTextFileResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as { fileName?: unknown; content?: unknown })
      : {};
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          relativePath?: unknown;
          size?: unknown;
          targetReferenceId?: unknown;
        })
      : null;

  if (result) {
    const relativePath = String(result.relativePath ?? argumentsObject.fileName ?? "新建文件");
    return (
      <div className="space-y-2">
        <p>
          {typeof result.directoryName === "string" ? `${result.directoryName} · ` : ""}
          已创建 {relativePath}
          {typeof result.size === "number" ? ` · ${formatFileSize(result.size)}` : ""}
        </p>
        <DirectoryEntryRow
          kind="file"
          label={relativePath}
          targetReferenceId={
            typeof result.targetReferenceId === "string" ? result.targetReferenceId : null
          }
        />
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <p>将在已授权目录根部创建 {String(argumentsObject.fileName ?? "新建文件")}</p>
      <div>
        <div className="mb-1 text-muted-foreground">内容预览</div>
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-2">
          {String(argumentsObject.content ?? "")}
        </pre>
      </div>
    </div>
  );
}

function CreateDirectoryResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as { directoryName?: unknown })
      : {};
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          relativePath?: unknown;
          targetReferenceId?: unknown;
        })
      : null;
  const relativePath = String(result?.relativePath ?? argumentsObject.directoryName ?? "新建目录");

  if (!result) {
    return <p>将在已授权目录根部创建目录 {relativePath}</p>;
  }

  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? `${result.directoryName} · ` : ""}
        已创建目录 {relativePath}
      </p>
      <DirectoryEntryRow
        kind="directory"
        label={relativePath}
        targetReferenceId={
          typeof result.targetReferenceId === "string" ? result.targetReferenceId : null
        }
      />
    </div>
  );
}

function CopyDirectoryEntryResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as {
          sourceRelativePath?: unknown;
          destinationDirectoryRelativePath?: unknown;
          destinationName?: unknown;
        })
      : {};
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          relativePath?: unknown;
          targetReferenceId?: unknown;
          kind?: unknown;
          copiedEntries?: unknown;
          copiedBytes?: unknown;
        })
      : null;

  if (!result) {
    const destination =
      typeof argumentsObject.destinationDirectoryRelativePath === "string"
        ? argumentsObject.destinationDirectoryRelativePath
        : "已授权目录根部";
    return (
      <div className="space-y-1">
        <p>来源：{String(argumentsObject.sourceRelativePath ?? "已授权条目")}</p>
        <p>
          目标：{destination} / {String(argumentsObject.destinationName ?? "副本")}
        </p>
        <p className="text-muted-foreground">
          不会覆盖同名条目；目录复制受条目数、深度和总大小限制。
        </p>
      </div>
    );
  }

  const relativePath = String(result.relativePath ?? argumentsObject.destinationName ?? "副本");
  const kind = result.kind === "directory" ? "directory" : "file";
  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? `${result.directoryName} · ` : ""}
        已复制到 {relativePath}
        {typeof result.copiedEntries === "number" ? ` · ${result.copiedEntries} 个条目` : ""}
        {typeof result.copiedBytes === "number" ? ` · ${formatFileSize(result.copiedBytes)}` : ""}
      </p>
      <DirectoryEntryRow
        kind={kind}
        label={relativePath}
        targetReferenceId={
          typeof result.targetReferenceId === "string" ? result.targetReferenceId : null
        }
      />
    </div>
  );
}

function MoveDirectoryEntryResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as {
          sourceRelativePath?: unknown;
          destinationDirectoryRelativePath?: unknown;
          destinationName?: unknown;
        })
      : {};
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          previousRelativePath?: unknown;
          relativePath?: unknown;
          targetReferenceId?: unknown;
          kind?: unknown;
        })
      : null;

  if (!result) {
    const destination =
      typeof argumentsObject.destinationDirectoryRelativePath === "string"
        ? argumentsObject.destinationDirectoryRelativePath
        : "已授权目录根部";
    return (
      <div className="space-y-1">
        <p>原位置：{String(argumentsObject.sourceRelativePath ?? "已授权条目")}</p>
        <p>
          新位置：{destination} / {String(argumentsObject.destinationName ?? "新名称")}
        </p>
        <p className="text-muted-foreground">不会覆盖同名条目；移动后原条目引用将失效。</p>
      </div>
    );
  }

  const relativePath = String(result.relativePath ?? argumentsObject.destinationName ?? "新位置");
  const kind = result.kind === "directory" ? "directory" : "file";
  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? result.directoryName + " · " : ""}
        已从 {String(
          result.previousRelativePath ?? argumentsObject.sourceRelativePath ?? "原位置",
        )}{" "}
        移动到 {relativePath}
      </p>
      <DirectoryEntryRow
        kind={kind}
        label={relativePath}
        targetReferenceId={
          typeof result.targetReferenceId === "string" ? result.targetReferenceId : null
        }
      />
    </div>
  );
}

function TrashDirectoryEntryResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as { relativePath?: unknown })
      : {};
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          relativePath?: unknown;
          kind?: unknown;
        })
      : null;
  const relativePath = String(result?.relativePath ?? argumentsObject.relativePath ?? "已授权条目");

  if (!result) {
    return (
      <div className="space-y-1">
        <p>将 {relativePath} 移入系统回收站</p>
        <p className="text-muted-foreground">不会永久删除；完成后可通过系统回收站恢复。</p>
      </div>
    );
  }

  return (
    <p>
      {typeof result.directoryName === "string" ? result.directoryName + " · " : ""}
      已将 {relativePath} 移入系统回收站
    </p>
  );
}

function BatchMoveDirectoryEntriesResult({
  argumentsValue,
  value,
}: {
  argumentsValue: unknown;
  value: unknown;
}) {
  const argumentsObject =
    argumentsValue && typeof argumentsValue === "object"
      ? (argumentsValue as {
          items?: unknown;
          conflictStrategy?: unknown;
        })
      : {};
  const argumentItems = Array.isArray(argumentsObject.items)
    ? (argumentsObject.items as Array<{
        sourceRelativePath?: unknown;
        destinationDirectoryRelativePath?: unknown;
        destinationName?: unknown;
      }>)
    : [];
  const result =
    value && typeof value === "object"
      ? (value as {
          directoryName?: unknown;
          moved?: unknown;
          skipped?: unknown;
          items?: unknown;
        })
      : null;

  if (!result) {
    return (
      <div className="space-y-2">
        <p>
          将批量处理 {argumentItems.length} 个条目 · 冲突时
          {argumentsObject.conflictStrategy === "skip" ? "跳过" : "整批停止"}
        </p>
        <div className="space-y-1">
          {argumentItems.map((item, index) => (
            <p key={index}>
              {String(item.sourceRelativePath ?? "已授权条目")} →{" "}
              {typeof item.destinationDirectoryRelativePath === "string"
                ? item.destinationDirectoryRelativePath + " / "
                : "已授权目录根部 / "}
              {String(item.destinationName ?? "新名称")}
            </p>
          ))}
        </div>
        <p className="text-muted-foreground">异常或取消时会回滚本批已经完成的移动。</p>
      </div>
    );
  }

  const resultItems = Array.isArray(result.items)
    ? (result.items as Array<{
        previousRelativePath?: unknown;
        relativePath?: unknown;
        kind?: unknown;
        status?: unknown;
        targetReferenceId?: unknown;
      }>)
    : [];
  return (
    <div className="space-y-2">
      <p>
        {typeof result.directoryName === "string" ? result.directoryName + " · " : ""}
        已移动 {Number(result.moved ?? 0)} 项 · 跳过 {Number(result.skipped ?? 0)} 项
      </p>
      <div className="space-y-2">
        {resultItems.map((item, index) =>
          item.status === "moved" ? (
            <DirectoryEntryRow
              key={index}
              kind={item.kind === "directory" ? "directory" : "file"}
              label={
                String(item.previousRelativePath ?? "原位置") +
                " → " +
                String(item.relativePath ?? "新位置")
              }
              targetReferenceId={
                typeof item.targetReferenceId === "string" ? item.targetReferenceId : null
              }
            />
          ) : (
            <p key={index} className="text-muted-foreground">
              已跳过 {String(item.previousRelativePath ?? "冲突条目")} →{" "}
              {String(item.relativePath ?? "冲突目标")}
            </p>
          ),
        )}
      </div>
    </div>
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
      file_already_exists: "同名文件或目录已存在，未执行覆盖。",
      file_name_invalid: "文件名无效；只能在授权目录根部创建普通 .txt 文件。",
      file_content_too_large: "文件内容超过 32 KiB 限制。",
      directory_name_invalid: "目录名无效；只能在授权目录根部创建单个普通目录。",
      file_copy_destination_invalid: "副本名称无效；请输入不含路径分隔符的普通名称。",
      file_copy_limit_exceeded: "复制范围超过 100 个条目、8 层目录或 16 MiB 限制。",
      file_copy_into_source: "不能把目录复制到自身或其子目录中。",
      file_move_destination_invalid: "新名称无效；请输入不含路径分隔符的普通名称。",
      file_move_into_source: "不能把目录移动到自身或其子目录中。",
      file_trash: "无法将该条目移入系统回收站；请检查系统权限或稍后重试。",
      file_batch_move_invalid: "批量移动必须包含 1 至 20 个互不重叠的条目。",
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
