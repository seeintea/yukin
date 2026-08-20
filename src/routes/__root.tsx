import type { QueryClient } from "@tanstack/react-query";
import { queryOptions } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, redirect } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

import { modelProviderList } from "#/api/model-provider";
import { WindowTitleBar } from "#/components/window-title-bar";
import { TanStackDevtools } from "#/integrations";
import { Toaster } from "#/shadcn/toast";

const providersQueryOptions = queryOptions({
  queryKey: ["model-provider", "list"],
  queryFn: modelProviderList,
  staleTime: Infinity,
});

const usesCustomWindowFrame = import.meta.env.TAURI_ENV_PLATFORM === "windows";

function WindowsAppFrame() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    let disposed = false;

    const updateMaximized = async () => {
      const maximized = await appWindow.isMaximized();

      if (!disposed) {
        setIsMaximized(maximized);
      }
    };

    void updateMaximized();
    void appWindow.onResized(updateMaximized).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        unlisten = stopListening;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div
      className="flex h-full flex-col overflow-hidden rounded-md border border-black/10 bg-background data-[maximized=true]:rounded-none dark:border-white/10"
      data-maximized={isMaximized}
    >
      <WindowTitleBar isMaximized={isMaximized} />
      <div className="min-h-0 flex-1">
        <Outlet />
      </div>
      <Toaster />
      <TanStackDevtools />
    </div>
  );
}

function AppFrame() {
  if (usesCustomWindowFrame) {
    return <WindowsAppFrame />;
  }

  return (
    <div className="h-full bg-background">
      <Outlet />
      <Toaster />
      <TanStackDevtools />
    </div>
  );
}

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient;
}>()({
  component: AppFrame,
  beforeLoad: async ({ context, location }) => {
    const providers = await context.queryClient.fetchQuery(providersQueryOptions);

    if (providers.length === 0 && location.pathname !== "/initialize") {
      throw redirect({ to: "/initialize" });
    }

    if (location.pathname === "/") {
      throw redirect({ to: "/chat" });
    }
  },
});
