import * as z from "zod";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "#/components/ui/card";
import { Button } from "#/components/ui/button";
import {
  ProviderForm,
  formSchema,
  type ProviderFormRef,
} from "#/components/provider-form";
import { useId, useRef, useState } from "react";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "#/components/ui/dialog";
import { streamDeepSeek } from "#/agent/providers/deep-seek";

export function Chat() {
  const providerId = useId();
  const providerRef = useRef<ProviderFormRef>({ reset: () => {} });
  const modalProviderId = useId();
  const [content, setContent] = useState("");

  async function onSubmit(data: z.infer<typeof formSchema>) {
    setContent("");
    for await (const delta of streamDeepSeek(data.baseUrl, data.key, [
      { role: "user", content: "你好，请介绍一下自己" },
    ])) {
      setContent((current) => current + delta);
    }
  }

  return (
    <div className={"flex items-center flex-col justify-center gap-4"}>
      <Card className={"w-lg"}>
        <CardHeader>
          <CardTitle>Agent 创建</CardTitle>
          <CardDescription>请输入供应商和密钥完成 Agent 创建。</CardDescription>
        </CardHeader>
        <CardContent>
          <ProviderForm id={providerId} onSubmit={onSubmit} ref={providerRef} />
        </CardContent>
        <CardFooter className={"flex items-center gap-4"}>
          <Button
            type="button"
            variant="outline"
            onClick={() => providerRef.current.reset()}
          >
            重置
          </Button>
          <Button type="submit" form={providerId}>
            创建 Agent
          </Button>
        </CardFooter>
      </Card>
      <Dialog>
        <DialogTrigger render={<Button variant="outline">创建 Agent</Button>} />
        <DialogContent className="w-96">
          <DialogHeader>
            <DialogTitle>Agent 创建</DialogTitle>
            <DialogDescription>
              请输入供应商和密钥完成 Agent 创建。
            </DialogDescription>
          </DialogHeader>
          <ProviderForm id={modalProviderId} onSubmit={onSubmit} />
          <DialogFooter>
            <DialogClose render={<Button variant="outline">关闭</Button>} />
            <Button type="submit" form={modalProviderId}>
              创建 Agent
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Card className={"w-lg"}>
        <CardHeader>Agent 输出</CardHeader>
        <CardContent>{content}</CardContent>
      </Card>
    </div>
  );
}
