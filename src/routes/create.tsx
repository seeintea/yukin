import { CreateScreen } from "#/features/create";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/create")({
  component: CreateScreen,
});
