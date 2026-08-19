import { createFileRoute } from "@tanstack/react-router";

import { ModelProviderSettings } from "#/features/model-provider";

export const Route = createFileRoute("/settings/providers")({
  component: ModelProviderSettings,
});
