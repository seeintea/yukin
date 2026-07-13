import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "#/components/ui/dialog";
import { Button } from "#/components/ui/button";
import { ProviderForm } from "#/components/provider-form";
import { useId } from "react";
import type { ProviderInput } from "#/domain/provider";

interface ProviderFormModalProps {
  onSubmit: (params: ProviderInput) => void;
}

export function ProviderFormModal(props: ProviderFormModalProps) {
  const id = useId();

  return (
    <Dialog>
      <DialogTrigger render={<Button variant="outline">创建 Agent</Button>} />
      <DialogContent className="w-lg">
        <DialogHeader>
          <DialogTitle>Agent 创建</DialogTitle>
          <DialogDescription>
            请输入供应商和密钥完成 Agent 创建。
          </DialogDescription>
        </DialogHeader>
        <ProviderForm id={id} onSubmit={props.onSubmit} />
        <DialogFooter>
          <DialogClose render={<Button variant="outline">关闭</Button>} />
          <Button type="submit" form={id}>
            创建 Agent
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
