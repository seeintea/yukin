import type { ProviderOutput } from "#/domain/provider";
import { ArrowUpIcon, SquareIcon } from "lucide-react";
import { useState } from "react";
import type { KeyboardEvent } from "react";
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
import { useAgent } from "#/hooks/use-agent";

interface ChatScreenProps {
  providers: ProviderOutput[];
}

export function ChatScreen({ providers }: ChatScreenProps) {
  const [providerId, setProviderId] = useState(providers[0]?.id ?? "");
  const [input, setInput] = useState("");
  const selectedProvider = providers.find(
    (provider) => provider.id === providerId,
  );
  const { messages, isRunning, sendMessage, stop } = useAgent(selectedProvider);
  const providerItems = providers.map((provider) => ({
    label: provider.providerAlias,
    value: provider.id,
  }));

  function submit() {
    if (!input.trim() || isRunning) return;

    const content = input;
    setInput("");
    void sendMessage(content);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (
      event.key !== "Enter" ||
      event.shiftKey ||
      event.nativeEvent.isComposing
    ) {
      return;
    }

    event.preventDefault();
    submit();
  }

  return (
    <div
      className={"h-full flex items-center flex-col justify-center gap-4 p-4"}
    >
      <div className={"flex-1 flex flex-col gap-2 w-full overflow-y-auto"}>
        {messages.map((message) =>
          message.role === "user" ? (
            <UserMessageBox key={message.id}>{message.content}</UserMessageBox>
          ) : (
            <div key={message.id}>
              <AIMessageBox>
                {message.content ||
                  (message.status === "streaming" ? "正在思考…" : "未生成内容")}
              </AIMessageBox>
              {message.error && (
                <p className="mt-1 text-xs text-destructive">{message.error}</p>
              )}
              {message.status === "cancelled" && (
                <p className="mt-1 text-xs text-muted-foreground">已停止生成</p>
              )}
            </div>
          ),
        )}
      </div>
      <InputGroup>
        <InputGroupTextarea
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="询问 yukin（Enter 发送，Shift + Enter 换行）"
          disabled={!selectedProvider}
        />
        <InputGroupAddon align="block-end">
          <Select
            value={providerId}
            onValueChange={(value) => setProviderId(value ?? "")}
            items={providerItems}
            disabled={isRunning}
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
            type="button"
            variant="default"
            size="icon-sm"
            className="ml-auto"
            onClick={isRunning ? stop : submit}
            disabled={!isRunning && (!input.trim() || !selectedProvider)}
            aria-label={isRunning ? "停止生成" : "发送消息"}
          >
            {isRunning ? <SquareIcon /> : <ArrowUpIcon />}
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </div>
  );
}
