import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { EyeIcon, MoreHorizontalIcon, PackageOpenIcon, PowerIcon, Trash2Icon } from "lucide-react";
import { useState } from "react";

import {
  mcpServerDelete,
  mcpServerImport,
  mcpServerList,
  mcpServerSetEnabled,
} from "#/api/mcp-server";
import type { McpServer, McpServerType } from "#/protocol/mcp-server";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/shadcn/alert-dialog";
import { Button } from "#/shadcn/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "#/shadcn/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";
import { Separator } from "#/shadcn/separator";
import { Skeleton } from "#/shadcn/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "#/shadcn/table";
import { toast } from "#/shadcn/toast";
import { showErrorToast } from "#/utils/toast";

const listKey = ["mcp-server", "list"] as const;
const typeLabels: Record<McpServerType, string> = {
  node: "Node.js",
  python: "Python",
  binary: "Binary",
  uv: "uv",
};

function upsert(servers: McpServer[], server: McpServer) {
  return [server, ...servers.filter((item) => item.id !== server.id)];
}

export function McpServerSettings() {
  const queryClient = useQueryClient();
  const serversQuery = useQuery({ queryKey: listKey, queryFn: mcpServerList, staleTime: Infinity });
  const [viewingServer, setViewingServer] = useState<McpServer | null>(null);
  const [deletingServer, setDeletingServer] = useState<McpServer | null>(null);

  const importMutation = useMutation({
    mutationFn: mcpServerImport,
    onSuccess: (server) => {
      if (!server) return;
      queryClient.setQueryData<McpServer[]>(listKey, (servers = []) => upsert(servers, server));
      toast.add({ title: "MCP Server 导入成功", type: "success" });
    },
    onError: (error) => showErrorToast("MCP Server 导入失败", error),
  });
  const enabledMutation = useMutation({
    mutationFn: mcpServerSetEnabled,
    onSuccess: (server) => {
      queryClient.setQueryData<McpServer[]>(listKey, (servers = []) => upsert(servers, server));
    },
    onError: (error) => showErrorToast("MCP Server 状态更新失败", error),
  });
  const deleteMutation = useMutation({
    mutationFn: mcpServerDelete,
    onSuccess: (_, request) => {
      queryClient.setQueryData<McpServer[]>(listKey, (servers = []) =>
        servers.filter((server) => server.id !== request.id),
      );
      setDeletingServer(null);
      toast.add({ title: "MCP Server 已删除", type: "success" });
    },
    onError: (error) => showErrorToast("MCP Server 删除失败", error),
  });

  const servers = serversQuery.data ?? [];
  return (
    <div className="h-full overflow-auto p-8">
      <div className="mx-auto flex max-w-5xl flex-col gap-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold">MCP Servers</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              导入本地 MCPB 并管理托管副本。当前不会启动 Server、安装依赖或发现动态 Tool。
            </p>
          </div>
          <Button disabled={importMutation.isPending} onClick={() => importMutation.mutate()}>
            <PackageOpenIcon />
            {importMutation.isPending ? "正在导入" : "导入 MCPB"}
          </Button>
        </div>

        <div className="overflow-hidden rounded-xl border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Server</TableHead>
                <TableHead>运行时</TableHead>
                <TableHead>声明 Tool</TableHead>
                <TableHead>状态</TableHead>
                <TableHead className="w-12" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {serversQuery.isPending ? (
                Array.from({ length: 3 }, (_, index) => (
                  <TableRow key={index}>
                    <TableCell colSpan={5}>
                      <Skeleton className="h-10 w-full" />
                    </TableCell>
                  </TableRow>
                ))
              ) : serversQuery.isError ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    MCP Server 加载失败
                  </TableCell>
                </TableRow>
              ) : servers.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    尚未导入 MCPB
                  </TableCell>
                </TableRow>
              ) : (
                servers.map((server) => (
                  <TableRow key={server.id}>
                    <TableCell className="max-w-md">
                      <div className="font-medium">{server.displayName ?? server.name}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {server.name} · v{server.version} · {server.authorName}
                      </div>
                    </TableCell>
                    <TableCell>{typeLabels[server.serverType]}</TableCell>
                    <TableCell>
                      {server.declaredTools.length > 0 ? server.declaredTools.length : "接入后发现"}
                    </TableCell>
                    <TableCell>{server.enabled ? "已启用" : "已停用"}</TableCell>
                    <TableCell>
                      <DropdownMenu>
                        <DropdownMenuTrigger
                          render={
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-sm"
                              aria-label="操作"
                            />
                          }
                        >
                          <MoreHorizontalIcon />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onClick={() => setViewingServer(server)}>
                            <EyeIcon />
                            查看详情
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            disabled={enabledMutation.isPending}
                            onClick={() =>
                              enabledMutation.mutate({ id: server.id, enabled: !server.enabled })
                            }
                          >
                            <PowerIcon />
                            {server.enabled ? "停用" : "启用"}
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setDeletingServer(server)}
                          >
                            <Trash2Icon />
                            删除
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>

      <Dialog
        open={viewingServer !== null}
        onOpenChange={(open) => !open && setViewingServer(null)}
      >
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{viewingServer?.displayName ?? viewingServer?.name}</DialogTitle>
            <DialogDescription>{viewingServer?.description}</DialogDescription>
          </DialogHeader>
          {viewingServer && (
            <div className="grid gap-5 text-sm">
              <div className="grid grid-cols-2 gap-4">
                <Detail label="标识" value={viewingServer.name} />
                <Detail label="版本" value={viewingServer.version} />
                <Detail label="作者" value={viewingServer.authorName} />
                <Detail label="运行时" value={typeLabels[viewingServer.serverType]} />
              </div>
              <Separator />
              <section>
                <h3 className="mb-2 font-medium">声明的 Tools</h3>
                {viewingServer.declaredTools.length === 0 ? (
                  <p className="text-muted-foreground">
                    manifest 未声明 Tool，将在接入 Server 后动态发现。
                  </p>
                ) : (
                  <div className="grid gap-2">
                    {viewingServer.declaredTools.map((tool) => (
                      <div key={tool.name} className="rounded-lg border p-3">
                        <div className="font-mono text-xs font-medium">{tool.name}</div>
                        {tool.description && (
                          <div className="mt-1 text-muted-foreground">{tool.description}</div>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </section>
              <section>
                <h3 className="mb-2 font-medium">配置字段</h3>
                {viewingServer.configFields.length === 0 ? (
                  <p className="text-muted-foreground">无需用户配置。</p>
                ) : (
                  <div className="grid gap-2">
                    {viewingServer.configFields.map((field) => (
                      <div
                        key={field.name}
                        className="flex items-start justify-between rounded-lg border p-3"
                      >
                        <div>
                          <div className="font-medium">{field.title}</div>
                          <div className="text-xs text-muted-foreground">
                            {field.name} · {field.fieldType}
                          </div>
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {[field.required && "必填", field.sensitive && "敏感"]
                            .filter(Boolean)
                            .join(" · ") || "可选"}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </section>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={deletingServer !== null}
        onOpenChange={(open) => !open && setDeletingServer(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除 MCP Server？</AlertDialogTitle>
            <AlertDialogDescription>
              将删除“{deletingServer?.displayName ?? deletingServer?.name}
              ”的记录和托管副本，不会影响最初下载的 MCPB。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMutation.isPending}>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteMutation.isPending}
              onClick={() => deletingServer && deleteMutation.mutate({ id: deletingServer.id })}
            >
              {deleteMutation.isPending ? "正在删除" : "删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 break-all">{value}</div>
    </div>
  );
}
