import { Link, Outlet } from "@tanstack/react-router";
import { ChevronLeftIcon, SettingsIcon, WaypointsIcon } from "lucide-react";

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
  return (
    <SidebarProvider className="h-svh min-h-0 overflow-hidden">
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
                  <SidebarMenuButton isActive render={<Link to="/settings/providers" />}>
                    <WaypointsIcon />
                    <span>模型供应商</span>
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
      <SidebarInset className="h-svh min-w-0 overflow-hidden">
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  );
}
