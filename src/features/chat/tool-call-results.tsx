import { useMutation } from "@tanstack/react-query";
import { ExternalLinkIcon, FileSearchIcon } from "lucide-react";

import { directoryEntryOpen, directoryEntryReveal } from "#/api/file";
import { Button } from "#/shadcn/button";
import { showErrorToast } from "#/utils/toast";

export function DirectorySearchResult({
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

export function DirectoryListResult({ value }: { value: unknown }) {
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

export function DirectoryEntryRow({
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

export function DirectoryEntryMetadataResult({ value }: { value: unknown }) {
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

export function DirectoryEntryActionResult({
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

export function FileReadResult({ value }: { value: unknown }) {
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

export function formatFileSize(size: number) {
  return size < 1024 ? `${size} B` : `${(size / 1024).toFixed(1)} KiB`;
}

function formatDateTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
