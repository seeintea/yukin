import { DirectoryEntryRow, formatFileSize } from "./tool-call-results";

export function CreateTextFileResult({
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

export function CreateDirectoryResult({
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

export function CopyDirectoryEntryResult({
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

export function MoveDirectoryEntryResult({
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

export function TrashDirectoryEntryResult({
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

export function BatchMoveDirectoryEntriesResult({
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
