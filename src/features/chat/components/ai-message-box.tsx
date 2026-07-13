import { PropsWithChildren } from "react";
import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import { Bubble, BubbleContent } from "#/components/ui/bubble";
import {
  Message,
  MessageAvatar,
  MessageContent,
} from "#/components/ui/message";

export function AIMessageBox(props: PropsWithChildren) {
  return (
    <Message>
      <MessageAvatar>
        <Avatar size="lg">
          <AvatarFallback className={"text-xs"}>Agent</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>
        <Bubble variant="muted" className={"max-w-sm"}>
          <BubbleContent>{props.children}</BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  );
}
