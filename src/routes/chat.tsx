import { createFileRoute } from "@tanstack/react-router";

import { Chat } from "#/features/chat";

export const Route = createFileRoute("/chat")({
  component: Chat,
});
