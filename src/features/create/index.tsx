import { useId, useRef } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import { ProviderForm, type ProviderFormRef } from "#/components/provider-form";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";
import { Button } from "#/components/ui/button";
import type { ProviderInput } from "#/domain/provider";
import { saveProvider } from "#/server/provider";

export function CreateScreen() {
  const id = useId();
  const ref = useRef<ProviderFormRef>({ reset: () => {} });
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { mutate } = useMutation({
    mutationFn: saveProvider,
    onSuccess: async (saved) => {
      if (!saved) {
        toast.error("创建 Agent 失败");
        return;
      }

      await queryClient.invalidateQueries({
        queryKey: ["provider", "list"],
      });
      toast.success("创建 Agent 成功");
      await navigate({ to: "/" });
    },
  });

  const onSubmit = (params: ProviderInput) => {
    mutate(params);
  };

  return (
    <div className={"h-full flex items-center flex-col justify-center"}>
      <Card className={"w-lg"}>
        <CardHeader>
          <CardTitle>Agent 创建</CardTitle>
          <CardDescription>请输入供应商和密钥完成 Agent 创建。</CardDescription>
        </CardHeader>
        <CardContent>
          <ProviderForm id={id} onSubmit={onSubmit} ref={ref} />
        </CardContent>
        <CardFooter className={"flex items-center gap-4"}>
          <Button
            type="button"
            variant="outline"
            onClick={() => ref.current.reset()}
          >
            重置
          </Button>
          <Button type="submit" form={id}>
            创建 Agent
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
