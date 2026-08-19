import { useMutation } from "@tanstack/react-query";
import { CircleCheckIcon } from "lucide-react";
import { useState } from "react";

import { modelProviderTestConnection } from "#/api/model-provider";
import type { ModelProvider, ModelProviderPreset } from "#/protocol/model-provider";
import { Button } from "#/shadcn/button";
import { Card, CardContent } from "#/shadcn/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "#/shadcn/dialog";
import { Field, FieldLabel } from "#/shadcn/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/shadcn/select";
import { getErrorMessage } from "#/utils/error";
import { showErrorToast } from "#/utils/toast";

interface ConnectionTestDialogProps {
  provider: ModelProvider;
  preset: ModelProviderPreset | undefined;
  onClose: () => void;
}

export function ConnectionTestDialog({ provider, preset, onClose }: ConnectionTestDialogProps) {
  const models =
    preset?.connections.find((connection) => connection.apiFormat === provider.apiFormat)?.models ??
    [];
  const [modelId, setModelId] = useState(models[0]?.modelId ?? "");
  const testMutation = useMutation({
    mutationFn: modelProviderTestConnection,
    onError: (error) => {
      showErrorToast("连接测试失败", error, "连接测试失败，请检查配置后重试");
    },
  });
  const modelItems = models.map((model) => ({
    label: model.displayName,
    value: model.modelId,
  }));
  const testedModel = models.find((model) => model.modelId === testMutation.data?.modelId);

  const handleOpenChange = (open: boolean) => {
    if (!open && !testMutation.isPending) {
      onClose();
    }
  };

  return (
    <Dialog open onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>测试“{provider.providerAlias}”连接</DialogTitle>
          <DialogDescription>
            将发送一次最小模型请求验证地址和 API Key，可能消耗少量 Token。
          </DialogDescription>
        </DialogHeader>

        <Field>
          <FieldLabel htmlFor="provider-test-model">测试模型</FieldLabel>
          <Select
            id="provider-test-model"
            value={modelId || null}
            onValueChange={(value) => {
              if (value) {
                testMutation.reset();
                setModelId(value);
              }
            }}
            items={modelItems}
            disabled={testMutation.isPending || modelItems.length === 0}
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="请选择测试模型" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {models.map((model) => (
                  <SelectItem key={model.modelId} value={model.modelId}>
                    {model.displayName}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>

        {testMutation.data && (
          <Card className="border-emerald-500/30 bg-emerald-500/5">
            <CardContent className="flex items-center gap-3 text-sm">
              <CircleCheckIcon className="size-5 text-emerald-600" />
              <div>
                <div className="font-medium">连接成功</div>
                <div className="text-muted-foreground">
                  {testedModel?.displayName ?? testMutation.data.modelId} ·{" "}
                  {testMutation.data.latencyMs} ms
                </div>
              </div>
            </CardContent>
          </Card>
        )}

        {testMutation.isError && (
          <p className="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {getErrorMessage(testMutation.error, "连接测试失败，请检查配置后重试")}
          </p>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={testMutation.isPending}
            onClick={onClose}
          >
            关闭
          </Button>
          <Button
            type="button"
            disabled={!modelId || testMutation.isPending}
            onClick={() => {
              testMutation.reset();
              testMutation.mutate({ providerId: provider.id, modelId });
            }}
          >
            {testMutation.isPending ? "正在测试" : "测试连接"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
