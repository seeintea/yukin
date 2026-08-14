import { createFileRoute } from "@tanstack/react-router";

import { Initialize } from "#/features/initialize";

export const Route = createFileRoute("/initialize")({
  component: Initialize,
});
