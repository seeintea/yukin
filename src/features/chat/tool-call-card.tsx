import type { ActiveToolCall } from "#/protocol/agent-run";
import { Button } from "#/shadcn/button";
import { Card, CardContent, CardHeader, CardTitle } from "#/shadcn/card";

import {
  BatchMoveDirectoryEntriesResult,
  CopyDirectoryEntryResult,
  CreateDirectoryResult,
  CreateTextFileResult,
  MoveDirectoryEntryResult,
  TrashDirectoryEntryResult,
} from "./tool-call-mutation-results";
import {
  DirectoryEntryActionResult,
  DirectoryEntryMetadataResult,
  DirectoryListResult,
  DirectorySearchResult,
  FileReadResult,
} from "./tool-call-results";

export function ToolCallCard({
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
