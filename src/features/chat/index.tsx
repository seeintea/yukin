import type { ProviderOutput } from "#/domain/provider";
import { ArrowUpIcon } from "lucide-react";
import {
  InputGroup,
  InputGroupTextarea,
  InputGroupAddon,
  InputGroupButton,
} from "#/components/ui/input-group";
import { UserMessageBox } from "./components/user-message-box";
import { AIMessageBox } from "./components/ai-message-box";

interface ChatScreenProps {
  providers: ProviderOutput[];
}

export function ChatScreen({ providers: _ }: ChatScreenProps) {
  return (
    <div
      className={"h-full flex items-center flex-col justify-center gap-4 p-4"}
    >
      <div className={"flex-1 flex flex-col gap-2 w-full"}>
        <UserMessageBox>Hi~ Agent</UserMessageBox>
        <AIMessageBox>Hi, User.</AIMessageBox>
      </div>
      <InputGroup>
        <InputGroupTextarea placeholder="询问 yukin" />
        <InputGroupAddon align="block-end">
          <InputGroupButton
            type="submit"
            variant="default"
            size="icon-sm"
            className="ml-auto"
          >
            <ArrowUpIcon />
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </div>
  );
}
