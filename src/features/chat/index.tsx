import type { ProviderOutput } from "#/domain/provider";
import { ArrowDownIcon, ArrowUpIcon, SquareIcon } from "lucide-react";
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
import { useChatScroll } from "#/hooks/use-chat-scroll";
import { Button } from "#/components/ui/button";

interface ChatScreenProps {
  providers: ProviderOutput[];
}

export function ChatScreen({ providers }: ChatScreenProps) {
  const [providerId, setProviderId] = useState(providers[0]?.id ?? "");
  const [input, setInput] = useState("");
  const selectedProvider = providers.find(
    (provider) => provider.id === providerId,
  );
  const {
    messages,
    activeRun,
    pendingRuns,
    enqueueUserMessage,
    cancelActiveRun,
  } = useAgent(selectedProvider);
  const {
    containerRef,
    isFollowing,
    scrollToBottom,
    handleScroll,
    handleWheel,
    handlePointerDown,
  } = useChatScroll(messages);
  const providerItems = providers.map((provider) => ({
    label: provider.providerAlias,
    value: provider.id,
  }));

  function submit() {
    if (!input.trim()) return;

    const content = input;
    setInput("");
    enqueueUserMessage(content);
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
      <div className="relative min-h-0 w-full flex-1">
        <div
          ref={containerRef}
          onScroll={handleScroll}
          onWheel={handleWheel}
          onPointerDown={handlePointerDown}
          className="absolute inset-0 flex flex-col gap-2 overflow-y-auto"
        >
          {messages.map((message) =>
            message.role === "user" ? (
              <UserMessageBox key={message.id}>
                {message.content.type === "text" ? message.content.text : ""}
              </UserMessageBox>
            ) : (
              <div key={message.id}>
                <AIMessageBox>
                  {(message.content.type === "text"
                    ? message.content.text
                    : `[交互消息：${message.content.name}]`) ||
                    (message.status === "streaming"
                      ? "正在思考…"
                      : "未生成内容")}
                </AIMessageBox>
                {message.error && (
                  <p className="mt-1 text-xs text-destructive">
                    {message.error}
                  </p>
                )}
                {message.status === "cancelled" && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    已停止生成
                  </p>
                )}
              </div>
            ),
          )}
        </div>
        {!isFollowing && (
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={scrollToBottom}
            className="absolute bottom-3 left-1/2 z-10 -translate-x-1/2 rounded-full shadow-md"
            aria-label="回到底部"
          >
            <ArrowDownIcon />
          </Button>
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
            disabled={activeRun !== null}
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
          {pendingRuns.length > 0 && (
            <span className="text-xs text-muted-foreground">
              等待中 {pendingRuns.length}
            </span>
          )}
          {activeRun && (
            <InputGroupButton
              type="button"
              variant="outline"
              size="icon-sm"
              onClick={cancelActiveRun}
              aria-label="停止生成"
            >
              <SquareIcon />
            </InputGroupButton>
          )}
          <InputGroupButton
            type="button"
            variant="default"
            size="icon-sm"
            className="ml-auto"
            onClick={submit}
            disabled={!input.trim() || !selectedProvider}
            aria-label={activeRun ? "加入等待队列" : "发送消息"}
          >
            <ArrowUpIcon />
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </div>
  );
}
