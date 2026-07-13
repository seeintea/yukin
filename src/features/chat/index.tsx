import type { ProviderOutput } from "#/domain/provider";
import { ArrowUpIcon } from "lucide-react";
import { useState } from "react";
import {
  InputGroup,
  InputGroupTextarea,
  InputGroupAddon,
  InputGroupButton,
} from "#/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { UserMessageBox } from "./components/user-message-box";
import { AIMessageBox } from "./components/ai-message-box";

interface ChatScreenProps {
  providers: ProviderOutput[];
}

export function ChatScreen({ providers }: ChatScreenProps) {
  const [providerId, setProviderId] = useState(providers[0]?.id ?? "");
  const providerItems = providers.map((provider) => ({
    label: provider.providerAlias,
    value: provider.id,
  }));

  return (
    <div
      className={"h-full flex items-center flex-col justify-center gap-4 p-4"}
    >
      <div className={"flex-1 flex flex-col gap-2 w-full"}>
        <UserMessageBox>Hi~ Agent</UserMessageBox>
        <AIMessageBox>
          {`# Hi, User.
- 列表1
- 列表2`}
        </AIMessageBox>
      </div>
      <InputGroup>
        <InputGroupTextarea placeholder="询问 yukin" />
        <InputGroupAddon align="block-end">
          <Select
            value={providerId}
            onValueChange={(value) => setProviderId(value ?? "")}
            items={providerItems}
          >
            <SelectTrigger size="sm">
              <SelectValue placeholder="选择 Provider" />
            </SelectTrigger>
            <SelectContent align="start">
              <SelectGroup>
                {providers.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id}>
                    {provider.providerAlias}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
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
