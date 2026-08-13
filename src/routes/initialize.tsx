import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/initialize")({
  component: RouteComponent,
});

function RouteComponent() {
  return <div>Hello "/initialize"!</div>;
}
