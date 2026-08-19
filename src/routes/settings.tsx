import { createFileRoute, redirect } from "@tanstack/react-router";

import { Settings } from "#/features/settings";

export const Route = createFileRoute("/settings")({
  component: Settings,
  beforeLoad: ({ location }) => {
    if (location.pathname === "/settings") {
      throw redirect({ to: "/settings/providers" });
    }
  },
});
