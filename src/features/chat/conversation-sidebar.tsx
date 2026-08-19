import { Link } from "@tanstack/react-router";
import {
  MessageSquareIcon,
  MoreHorizontalIcon,
  PencilIcon,
  SettingsIcon,
  SquarePenIcon,
  Trash2Icon,
} from "lucide-react";
import { useState } from "react";

import type { Conversation } from "#/protocol/conversation";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/shadcn/alert-dialog";
import { Button } from "#/shadcn/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "#/shadcn/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";
import { Input } from "#/shadcn/input";
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
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/shadcn/sidebar";

interface ConversationSidebarProps {
  conversations: Conversation[];
  selectedConversationId: string;
  isCreating: boolean;
  onCreate: () => void;
  onSelect: (conversationId: string) => void;
  onRename: (conversationId: string, title: string) => Promise<void>;
  onDelete: (conversationId: string) => Promise<void>;
  renamingConversationId: string | null;
  deletingConversationId: string | null;
}

export function ConversationSidebar({
  conversations,
  selectedConversationId,
  isCreating,
  onCreate,
  onSelect,
  onRename,
  onDelete,
  renamingConversationId,
  deletingConversationId,
}: ConversationSidebarProps) {
  const [renamingConversation, setRenamingConversation] = useState<Conversation | null>(null);
  const [deletingConversation, setDeletingConversation] = useState<Conversation | null>(null);
  const [title, setTitle] = useState("");
  const isRenaming = renamingConversationId === renamingConversation?.id;
  const isDeleting = deletingConversationId === deletingConversation?.id;

  const openRename = (conversation: Conversation) => {
    setTitle(conversation.title);
    setRenamingConversation(conversation);
  };
  const submitRename = () => {
    if (!renamingConversation || !title.trim() || isRenaming) {
      return;
    }
    void onRename(renamingConversation.id, title)
      .then(() => setRenamingConversation(null))
      .catch(() => undefined);
  };
  const submitDelete = () => {
    if (!deletingConversation || isDeleting) {
      return;
    }
    void onDelete(deletingConversation.id)
      .then(() => setDeletingConversation(null))
      .catch(() => undefined);
  };

  return (
    <>
      <Sidebar collapsible="none">
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                variant="outline"
                size="lg"
                disabled={isCreating}
                onClick={onCreate}
              >
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
                        className="pr-8"
                        isActive={conversation.id === selectedConversationId}
                        tooltip={conversation.title}
                        onClick={() => onSelect(conversation.id)}
                      >
                        <MessageSquareIcon />
                        <span>{conversation.title}</span>
                      </SidebarMenuButton>
                      <DropdownMenu>
                        <DropdownMenuTrigger
                          render={
                            <SidebarMenuAction
                              showOnHover
                              aria-label={`${conversation.title} 操作`}
                            />
                          }
                        >
                          <MoreHorizontalIcon />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="start" side="right">
                          <DropdownMenuItem onClick={() => openRename(conversation)}>
                            <PencilIcon />
                            重命名
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setDeletingConversation(conversation)}
                          >
                            <Trash2Icon />
                            删除
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
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

      <Dialog
        open={renamingConversation !== null}
        onOpenChange={(open) => !open && !isRenaming && setRenamingConversation(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>重命名会话</DialogTitle>
            <DialogDescription>输入一个便于识别的会话标题。</DialogDescription>
          </DialogHeader>
          <form
            id="conversation-rename-form"
            onSubmit={(event) => {
              event.preventDefault();
              submitRename();
            }}
          >
            <Input
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              maxLength={120}
              disabled={isRenaming}
              autoFocus
            />
          </form>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={isRenaming}
              onClick={() => setRenamingConversation(null)}
            >
              取消
            </Button>
            <Button
              type="submit"
              form="conversation-rename-form"
              disabled={!title.trim() || isRenaming}
            >
              {isRenaming ? "正在保存" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={deletingConversation !== null}
        onOpenChange={(open) => !open && !isDeleting && setDeletingConversation(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除“{deletingConversation?.title}”？</AlertDialogTitle>
            <AlertDialogDescription>
              该会话的消息、Run 和 Tool Call 将一并删除，此操作无法撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>取消</AlertDialogCancel>
            <AlertDialogAction variant="destructive" disabled={isDeleting} onClick={submitDelete}>
              {isDeleting ? "正在删除" : "删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
