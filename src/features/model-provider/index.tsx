import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRoundIcon, MoreHorizontalIcon, PencilIcon, PlusIcon, Trash2Icon } from "lucide-react";
import { useId, useRef, useState } from "react";

import {
  modelProviderCreate,
  modelProviderCredentialReplace,
  modelProviderDelete,
  modelProviderUpdate,
} from "#/api/model-provider";
import { ModelProviderForm } from "#/components/model-provider-form";
import type { ModelProviderFormRef } from "#/components/model-provider-form";
import type {
  ApiFormat,
  CreateRequest,
  ModelProvider,
  ReplaceCredentialRequest,
  UpdateRequest,
} from "#/protocol/model-provider";
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
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "#/shadcn/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";
import { Skeleton } from "#/shadcn/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "#/shadcn/table";
import { toast } from "#/shadcn/toast";

import { ProviderCredentialForm, ProviderUpdateForm } from "./forms";
import {
  modelProviderKeys,
  modelProviderListQueryOptions,
  modelProviderPresetListQueryOptions,
} from "./queries";

const apiFormatLabels: Record<ApiFormat, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
};

function getErrorMessage(error: unknown) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "请稍后重试";
}

function formatUpdatedAt(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function ModelProviderSettings() {
  const queryClient = useQueryClient();
  const providersQuery = useQuery(modelProviderListQueryOptions);
  const presetsQuery = useQuery(modelProviderPresetListQueryOptions);
  const createFormId = useId();
  const updateFormId = useId();
  const credentialFormId = useId();
  const createFormRef = useRef<ModelProviderFormRef>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<ModelProvider | null>(null);
  const [credentialProvider, setCredentialProvider] = useState<ModelProvider | null>(null);
  const [deletingProvider, setDeletingProvider] = useState<ModelProvider | null>(null);

  const refreshProviders = () =>
    queryClient.invalidateQueries({ queryKey: modelProviderKeys.list });
  const createMutation = useMutation({
    mutationFn: modelProviderCreate,
    onSuccess: async () => {
      await refreshProviders();
      setCreateOpen(false);
      createFormRef.current?.reset();
      toast.add({ title: "模型供应商创建成功", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "模型供应商创建失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });
  const updateMutation = useMutation({
    mutationFn: modelProviderUpdate,
    onSuccess: async () => {
      await refreshProviders();
      setEditingProvider(null);
      toast.add({ title: "模型供应商更新成功", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "模型供应商更新失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });
  const credentialMutation = useMutation({
    mutationFn: modelProviderCredentialReplace,
    onSuccess: () => {
      setCredentialProvider(null);
      toast.add({ title: "API Key 更新成功", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "API Key 更新失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });
  const deleteMutation = useMutation({
    mutationFn: modelProviderDelete,
    onSuccess: async () => {
      await refreshProviders();
      setDeletingProvider(null);
      toast.add({ title: "模型供应商已删除", type: "success" });
    },
    onError: (error) => {
      toast.add({
        title: "模型供应商删除失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });

  const providers = providersQuery.data ?? [];
  const getProviderName = (provider: ModelProvider) =>
    presetsQuery.data?.find((preset) => preset.providerKey === provider.providerKey)?.displayName ??
    provider.providerKey;
  const getApiFormats = (provider: ModelProvider) => {
    const formats = presetsQuery.data
      ?.find((preset) => preset.providerKey === provider.providerKey)
      ?.connections.map((connection) => connection.apiFormat);
    return formats && formats.length > 0 ? formats : [provider.apiFormat];
  };

  const handleCreateOpenChange = (open: boolean) => {
    if (!open && createMutation.isPending) {
      return;
    }
    if (!open) {
      createFormRef.current?.reset();
    }
    setCreateOpen(open);
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto">
      <div className="mx-auto w-full max-w-6xl px-8 py-10">
        <div className="mb-8 flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">模型供应商</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              管理模型服务地址、兼容格式和访问密钥。
            </p>
          </div>
          <Dialog open={createOpen} onOpenChange={handleCreateOpenChange}>
            <DialogTrigger render={<Button />}>
              <PlusIcon />
              新建供应商
            </DialogTrigger>
            <DialogContent className="sm:max-w-lg">
              <DialogHeader>
                <DialogTitle>新建模型供应商</DialogTitle>
                <DialogDescription>添加一个可用于聊天的模型服务配置。</DialogDescription>
              </DialogHeader>
              <ModelProviderForm
                id={createFormId}
                ref={createFormRef}
                onSubmit={(request: CreateRequest) => createMutation.mutate(request)}
              />
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  disabled={createMutation.isPending}
                  onClick={() => handleCreateOpenChange(false)}
                >
                  取消
                </Button>
                <Button type="submit" form={createFormId} disabled={createMutation.isPending}>
                  {createMutation.isPending ? "正在创建" : "创建"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        <div className="overflow-hidden rounded-xl border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>名称</TableHead>
                <TableHead>API 格式</TableHead>
                <TableHead>请求地址</TableHead>
                <TableHead>更新时间</TableHead>
                <TableHead className="w-12" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {providersQuery.isPending ? (
                Array.from({ length: 3 }, (_, index) => (
                  <TableRow key={index}>
                    <TableCell colSpan={5}>
                      <Skeleton className="h-8 w-full" />
                    </TableCell>
                  </TableRow>
                ))
              ) : providersQuery.isError ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    模型供应商加载失败
                  </TableCell>
                </TableRow>
              ) : providers.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    暂无模型供应商
                  </TableCell>
                </TableRow>
              ) : (
                providers.map((provider) => (
                  <TableRow key={provider.id}>
                    <TableCell>
                      <div className="font-medium">{provider.providerAlias}</div>
                      <div className="text-xs text-muted-foreground">
                        {getProviderName(provider)}
                      </div>
                    </TableCell>
                    <TableCell>{apiFormatLabels[provider.apiFormat]}</TableCell>
                    <TableCell className="max-w-80 truncate font-mono text-xs">
                      {provider.baseUrl}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatUpdatedAt(provider.updatedAt)}
                    </TableCell>
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
                          <DropdownMenuItem onClick={() => setEditingProvider(provider)}>
                            <PencilIcon />
                            编辑配置
                          </DropdownMenuItem>
                          <DropdownMenuItem onClick={() => setCredentialProvider(provider)}>
                            <KeyRoundIcon />
                            替换 API Key
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setDeletingProvider(provider)}
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
        open={editingProvider !== null}
        onOpenChange={(open) => !open && !updateMutation.isPending && setEditingProvider(null)}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>编辑模型供应商</DialogTitle>
            <DialogDescription>更新别名、兼容格式或模型服务地址。</DialogDescription>
          </DialogHeader>
          {editingProvider && (
            <ProviderUpdateForm
              key={editingProvider.id}
              id={updateFormId}
              provider={editingProvider}
              apiFormats={getApiFormats(editingProvider)}
              onSubmit={(request: UpdateRequest) => updateMutation.mutate(request)}
            />
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={updateMutation.isPending}
              onClick={() => setEditingProvider(null)}
            >
              取消
            </Button>
            <Button type="submit" form={updateFormId} disabled={updateMutation.isPending}>
              {updateMutation.isPending ? "正在保存" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={credentialProvider !== null}
        onOpenChange={(open) =>
          !open && !credentialMutation.isPending && setCredentialProvider(null)
        }
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>替换 API Key</DialogTitle>
            <DialogDescription>
              为“{credentialProvider?.providerAlias}”设置新的访问密钥。
            </DialogDescription>
          </DialogHeader>
          {credentialProvider && (
            <ProviderCredentialForm
              key={credentialProvider.id}
              id={credentialFormId}
              providerId={credentialProvider.id}
              onSubmit={(request: ReplaceCredentialRequest) => credentialMutation.mutate(request)}
            />
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={credentialMutation.isPending}
              onClick={() => setCredentialProvider(null)}
            >
              取消
            </Button>
            <Button type="submit" form={credentialFormId} disabled={credentialMutation.isPending}>
              {credentialMutation.isPending ? "正在更新" : "更新密钥"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={deletingProvider !== null}
        onOpenChange={(open) => !open && !deleteMutation.isPending && setDeletingProvider(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除“{deletingProvider?.providerAlias}”？</AlertDialogTitle>
            <AlertDialogDescription>
              该供应商配置和对应的 API Key 将被永久删除，此操作无法撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMutation.isPending}>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteMutation.isPending}
              onClick={() => deletingProvider && deleteMutation.mutate({ id: deletingProvider.id })}
            >
              {deleteMutation.isPending ? "正在删除" : "删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
