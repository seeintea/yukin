import { Link } from "@tanstack/react-router";
import { MessageSquareIcon, SettingsIcon, SquarePenIcon } from "lucide-react";

import type { Conversation } from "#/protocol/conversation";
import { ScrollArea } from "#/shadcn/scroll-area";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/shadcn/sidebar";

interface ConversationSidebarProps {
  conversations: Conversation[];
  selectedConversationId: string;
  isCreating: boolean;
  onCreate: () => void;
  onSelect: (conversationId: string) => void;
}

export function ConversationSidebar({
  conversations,
  selectedConversationId,
  isCreating,
  onCreate,
  onSelect,
}: ConversationSidebarProps) {
  return (
    <Sidebar collapsible="none">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton variant="outline" size="lg" disabled={isCreating} onClick={onCreate}>
              <SquarePenIcon />
              <span>{isCreating ? "正在创建" : "新对话"}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent className="overflow-hidden">
        <ScrollArea className="min-h-0 flex-1">
          <SidebarGroup>
            <SidebarGroupLabel>最近</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {conversations.map((conversation) => (
                  <SidebarMenuItem key={conversation.id}>
                    <SidebarMenuButton
                      isActive={conversation.id === selectedConversationId}
                      tooltip={conversation.title}
                      onClick={() => onSelect(conversation.id)}
                    >
                      <MessageSquareIcon />
                      <span>{conversation.title}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </ScrollArea>
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton render={<Link to="/settings/providers" />}>
              <SettingsIcon />
              <span>设置</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
