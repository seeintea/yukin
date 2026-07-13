import { ChatScreen } from "#/features/chat";
import { getProviders } from "#/server/provider";
import { queryOptions } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

const providersQueryOptions = queryOptions({
  queryKey: ["provider", "list"],
  queryFn: getProviders,
});

export const Route = createFileRoute("/")({
  loader: async ({ context }) => {
    const providers = await context.queryClient.fetchQuery(
      providersQueryOptions,
    );

    if (providers.length === 0) {
      throw redirect({ to: "/create" });
    }

    return { providers };
  },
  component: IndexPage,
});

function IndexPage() {
  const { providers } = Route.useLoaderData();

  return <ChatScreen providers={providers} />;
}
