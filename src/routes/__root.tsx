import type { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, redirect } from "@tanstack/react-router";

import { TanStackDevtools } from "#/integrations";
import { Toaster } from "#/shadcn/toast";

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient;
}>()({
  component: () => (
    <>
      <Outlet />
      <Toaster />
      <TanStackDevtools />
    </>
  ),
  beforeLoad: () => {
    if (location.pathname !== "/chat") {
      throw redirect({
        to: "/chat",
      });
    }
  },
});
