import { createFileRoute } from "@tanstack/react-router";

import { McpServerSettings } from "#/features/mcp-server";

export const Route = createFileRoute("/settings/mcp-servers")({
  component: McpServerSettings,
});
