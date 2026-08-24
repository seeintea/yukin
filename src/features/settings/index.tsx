import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import {
  BlocksIcon,
  BookOpenIcon,
  ChevronLeftIcon,
  SettingsIcon,
  WaypointsIcon,
} from "lucide-react";

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "#/shadcn/sidebar";

export function Settings() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });

  return (
    <SidebarProvider className="h-full min-h-0 overflow-hidden">
      <Sidebar collapsible="none">
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton size="lg">
                <SettingsIcon />
                <span className="font-medium">设置</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupLabel>模型</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={pathname === "/settings/providers"}
                    render={<Link to="/settings/providers" />}
                  >
                    <WaypointsIcon />
                    <span>模型供应商</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
          <SidebarGroup>
            <SidebarGroupLabel>扩展</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={pathname === "/settings/skills"}
                    render={<Link to="/settings/skills" />}
                  >
                    <BookOpenIcon />
                    <span>Skills</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={pathname === "/settings/mcp-servers"}
                    render={<Link to="/settings/mcp-servers" />}
                  >
                    <BlocksIcon />
                    <span>MCP Servers</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton render={<Link to="/chat" />}>
                <ChevronLeftIcon />
                <span>返回对话</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset className="h-full min-w-0 overflow-hidden">
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  );
}
