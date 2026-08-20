import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";

interface WindowTitleBarProps {
  isMaximized: boolean;
}

const appWindow = getCurrentWindow();

export function WindowTitleBar({ isMaximized }: WindowTitleBarProps) {
  return (
    <header className="flex h-10 shrink-0 select-none items-center border-b border-black/5 bg-background dark:border-white/5">
      <div
        className="flex h-full min-w-0 flex-1 items-center px-3 text-sm font-medium"
        data-tauri-drag-region
      >
        <span className="truncate" data-tauri-drag-region>
          yukin
        </span>
      </div>

      <div className="flex h-full shrink-0">
        <button
          type="button"
          className="flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground dark:hover:bg-white/10"
          title="最小化"
          onClick={() => void appWindow.minimize()}
        >
          <Minus className="size-4" />
        </button>
        <button
          type="button"
          className="flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground dark:hover:bg-white/10"
          title={isMaximized ? "还原" : "最大化"}
          onClick={() => void appWindow.toggleMaximize()}
        >
          {isMaximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
        </button>
        <button
          type="button"
          className="flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-red-500 hover:text-white"
          title="关闭"
          onClick={() => void appWindow.close()}
        >
          <X className="size-4" />
        </button>
      </div>
    </header>
  );
}
