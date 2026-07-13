import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import { Bubble, BubbleContent } from "#/components/ui/bubble";
import {
  Message,
  MessageAvatar,
  MessageContent,
} from "#/components/ui/message";
import { MarkdownMessage } from "./markdown-message";

interface AIMessageBoxProps {
  children: string;
}

export function AIMessageBox({ children }: AIMessageBoxProps) {
  return (
    <Message>
      <MessageAvatar>
        <Avatar size="lg">
          <AvatarFallback className={"text-xs"}>Agent</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>
        <Bubble variant="muted" className={"max-w-sm"}>
          <BubbleContent>
            <MarkdownMessage>{children}</MarkdownMessage>
          </BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  );
}
