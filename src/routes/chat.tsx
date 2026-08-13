import { createFileRoute } from "@tanstack/react-router";

import { Button } from "#/shadcn/button";

export const Route = createFileRoute("/chat")({
  component: Index,
});

function Index() {
  return (
    <div className="p-2">
      <Button>123</Button>
    </div>
  );
}
