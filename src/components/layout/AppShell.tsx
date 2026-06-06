import { Link, useLocation } from "react-router";
import { MessageSquare, Settings as SettingsIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { Separator } from "@/components/ui/separator";

interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps) {
  const location = useLocation();
  const isSettings = location.pathname.startsWith("/settings");
  const isChat = !isSettings;

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-sidebar text-sidebar-foreground">
        <div className="px-4 py-3">
          <h1 className="text-lg font-semibold">Yukin</h1>
        </div>
        <Separator />
        <nav className="flex flex-col gap-1 p-2">
          <NavItem to="/chat/new" icon={<MessageSquare size={16} />} active={isChat}>
            Chat
          </NavItem>
          <NavItem to="/settings" icon={<SettingsIcon size={16} />} active={isSettings}>
            Settings
          </NavItem>
        </nav>
        <Separator />
        {/* 会话列表占位 — Phase E/I 填充 */}
        <div className="flex-1 overflow-y-auto p-2 text-xs text-muted-foreground">
          Sessions will appear here
        </div>
      </aside>
      <main className="flex flex-1 flex-col overflow-hidden">{children}</main>
    </div>
  );
}

interface NavItemProps {
  to: string;
  icon: React.ReactNode;
  active: boolean;
  children: React.ReactNode;
}

function NavItem({ to, icon, active, children }: NavItemProps) {
  return (
    <Link
      to={to}
      className={cn(
        "flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
        active
          ? "bg-sidebar-accent text-sidebar-accent-foreground"
          : "text-sidebar-foreground hover:bg-sidebar-accent/50",
      )}
    >
      {icon}
      {children}
    </Link>
  );
}
