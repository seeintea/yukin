import { PropsWithChildren } from "react";
import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import { Bubble, BubbleContent } from "#/components/ui/bubble";
import {
  Message,
  MessageAvatar,
  MessageContent,
} from "#/components/ui/message";

export function UserMessageBox(props: PropsWithChildren) {
  return (
    <Message align="end">
      <MessageAvatar>
        <Avatar size="lg">
          <AvatarFallback className={"text-xs"}>User</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>
        <Bubble align="end" className={"max-w-sm"}>
          <BubbleContent>{props.children}</BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  );
}
