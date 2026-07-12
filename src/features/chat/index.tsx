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
import { DeepSeekError, streamDeepSeek } from "#/agent/providers/deep-seek";
import type { DeepSeekFinishReason } from "#/agent/providers/deep-seek/types";

type OutputStatus =
  | "idle" // 尚未请求
  | "streaming" // 正在接收内容
  | "complete" // 收到可信的 finish 事件
  | "incomplete" // 已有部分内容，但随后发生错误
  | "failed"; // 尚未收到任何内容就发生错误

interface AgentOutput {
  status: OutputStatus;
  content: string;
  error: string | null;
  finishReason: DeepSeekFinishReason | null;
}

export function Chat() {
  const providerId = useId();
  const providerRef = useRef<ProviderFormRef>({ reset: () => {} });
  const isRunningRef = useRef(false);
  const modalProviderId = useId();
  const [agentOutput, setAgentOutput] = useState<AgentOutput>(() => ({
    status: "idle",
    content: "",
    error: null,
    finishReason: null,
  }));
  const isStreaming = agentOutput.status === "streaming";

  async function onSubmit(data: z.infer<typeof formSchema>) {
    if (isRunningRef.current) return;

    isRunningRef.current = true;
    setAgentOutput({
      status: "streaming",
      content: "",
      error: null,
      finishReason: null,
    });

    try {
      for await (const event of streamDeepSeek(data.baseUrl, data.key, [
        { role: "user", content: "你好，请介绍一下自己" },
      ])) {
        switch (event.type) {
          case "content":
            setAgentOutput((current) => ({
              ...current,
              status: "streaming",
              content: current.content + event.content,
            }));
            break;

          case "finish":
            setAgentOutput((current) => ({
              ...current,
              status: "complete",
              finishReason: event.reason,
            }));
            break;
        }
      }
    } catch (cause) {
      if (cause instanceof DeepSeekError) {
        setAgentOutput((current) => ({
          ...current,
          status: current.content.length > 0 ? "incomplete" : "failed",
          error: `[${cause.code}] ${cause.message}`,
        }));
        return;
      }

      setAgentOutput((current) => ({
        ...current,
        status: current.content.length > 0 ? "incomplete" : "failed",
        error: "发生未知错误",
      }));
    } finally {
      isRunningRef.current = false;
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
            disabled={isStreaming}
            onClick={() => providerRef.current.reset()}
          >
            重置
          </Button>
          <Button type="submit" form={providerId} disabled={isStreaming}>
            创建 Agent
          </Button>
        </CardFooter>
      </Card>
      <Dialog>
        <DialogTrigger
          render={
            <Button variant="outline" disabled={isStreaming}>
              创建 Agent
            </Button>
          }
        />
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
            <Button type="submit" form={modalProviderId} disabled={isStreaming}>
              创建 Agent
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Card className={"w-lg"}>
        <CardHeader>Agent 输出</CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            状态：{agentOutput.status}
            {agentOutput.finishReason &&
              `，结束原因：${agentOutput.finishReason}`}
          </p>
          {agentOutput.content && <p>{agentOutput.content}</p>}
          {agentOutput.error && (
            <p className="text-destructive">{agentOutput.error}</p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
