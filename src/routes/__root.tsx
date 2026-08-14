import type { QueryClient } from "@tanstack/react-query";
import { queryOptions } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, redirect } from "@tanstack/react-router";

import { modelProviderList } from "#/api/model-provider";
import { TanStackDevtools } from "#/integrations";
import { Toaster } from "#/shadcn/toast";

const providersQueryOptions = queryOptions({
  queryKey: ["model-provider", "list"],
  queryFn: modelProviderList,
  staleTime: Infinity,
});

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
