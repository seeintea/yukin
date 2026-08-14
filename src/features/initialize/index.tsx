import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useId, useRef } from "react";

import { modelProviderCreate } from "#/api/model-provider";
import { ModelProviderForm } from "#/components/model-provider-form";
import type { ModelProviderFormRef } from "#/components/model-provider-form";
import type { CreateRequest, ModelProvider } from "#/protocol/model-provider";
import { Button } from "#/shadcn/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "#/shadcn/card";
import { toast } from "#/shadcn/toast";

const modelProviderListQueryKey = ["model-provider", "list"] as const;

function getErrorMessage(error: unknown) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "请检查配置后重试";
}

export function Initialize() {
  const formId = useId();
  const formRef = useRef<ModelProviderFormRef>(null);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const createMutation = useMutation({
    mutationFn: modelProviderCreate,
    onSuccess: async (provider) => {
      queryClient.setQueryData<ModelProvider[]>(modelProviderListQueryKey, (providers = []) => [
        provider,
        ...providers.filter((item) => item.id !== provider.id),
      ]);
      toast.add({
        title: "模型供应商创建成功",
        type: "success",
      });
      await navigate({ to: "/chat" });
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

  const handleSubmit = (request: CreateRequest) => {
    createMutation.mutate(request);
  };

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle>配置模型供应商</CardTitle>
          <CardDescription>选择供应商并填写 API Key，完成初始模型接入。</CardDescription>
        </CardHeader>
        <CardContent>
          <ModelProviderForm id={formId} onSubmit={handleSubmit} ref={formRef} />
        </CardContent>
        <CardFooter className="justify-end gap-3">
          <Button
            type="button"
            variant="outline"
            disabled={createMutation.isPending}
            onClick={() => formRef.current?.reset()}
          >
            重置
          </Button>
          <Button type="submit" form={formId} disabled={createMutation.isPending}>
            {createMutation.isPending ? "正在创建" : "创建供应商"}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
