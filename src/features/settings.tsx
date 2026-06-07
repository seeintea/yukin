import { Button } from "#/components/ui/button";

export function SettingsScreen() {
  return (
    <div className="flex flex-1 flex-col gap-4 overflow-y-auto p-6">
      <header>
        <h2 className="text-2xl font-semibold">Settings</h2>
        <p className="text-sm text-muted-foreground">
          Configure your workspace, provider, and API key.
        </p>
      </header>
      <section className="space-y-2 rounded-lg border border-border p-4">
        <h3 className="font-medium">Placeholder</h3>
        <p className="text-sm text-muted-foreground">
          Workspace selector, API key form, and provider picker will live here (Phase D + E).
        </p>
        <Button size="sm" variant="secondary">
          Coming soon
        </Button>
      </section>
    </div>
  );
}