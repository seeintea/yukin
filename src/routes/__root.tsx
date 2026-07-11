import { TanStackDevtools } from "#/integrations";
import { Outlet, createRootRoute } from "@tanstack/react-router";
import { Toaster } from "#/components/ui/sonner";

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  return (
    <>
      <Outlet />
      <Toaster />
      <TanStackDevtools />
    </>
  );
}
