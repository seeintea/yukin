import { TanStackDevtools as Devtools } from "@tanstack/react-devtools";
import { ReactQueryDevtoolsPanel } from "@tanstack/react-query-devtools";
import { TanStackRouterDevtoolsPanel } from "@tanstack/react-router-devtools";

const plugins = [
  { name: "Router", render: <TanStackRouterDevtoolsPanel /> },
  { name: "Query", render: <ReactQueryDevtoolsPanel /> },
];

export function TanStackDevtools() {
  return <Devtools config={{ position: "bottom-right" }} plugins={plugins} />;
}
