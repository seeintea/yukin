import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "#/components/ui/button";

interface MemoryRow {
  id: string;
  name: string;
  kind: string;
  description: string | null;
  content: string;
  metadata: string;
  workspace: string | null;
  createdAt: string;
  updatedAt: string;
}

interface SmokeStep {
  label: string;
  ok: boolean;
  data: unknown;
}

export function SettingsScreen() {
  const [steps, setSteps] = useState<SmokeStep[]>([]);
  const [running, setRunning] = useState(false);

  async function runMemorySmokeTest() {
    setRunning(true);
    setSteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setSteps((prev) => [...prev, { label, ok, data }]);

    try {
      // 1. save
      const saved = await invoke<MemoryRow>("memory_save", {
        input: {
          name: "smoke-test",
          kind: "user",
          content: "Hello memory layer!",
          metadata: { source: "settings-smoke-test" },
        },
      });
      log("memory_save", true, saved);

      // 2. recall (should hit)
      const hits1 = await invoke<MemoryRow[]>("memory_recall", {
        query: "Hello",
      });
      log(`memory_recall("Hello") → ${hits1.length} hit(s)`, hits1.length === 1, hits1);

      // 3. list (should contain saved)
      const all = await invoke<MemoryRow[]>("memory_list", {});
      const inList = all.some((m) => m.id === saved.id);
      log(`memory_list → ${all.length} total, contains saved: ${inList}`, inList, all);

      // 4. update (change content)
      const updated = await invoke<MemoryRow>("memory_update", {
        id: saved.id,
        patch: { content: "Hello updated content" },
      });
      log("memory_update content → 'Hello updated content'", updated.content === "Hello updated content", updated);

      // 5. recall new content (should hit)
      const hits2 = await invoke<MemoryRow[]>("memory_recall", {
        query: "updated",
      });
      log(`memory_recall("updated") → ${hits2.length} hit(s)`, hits2.length === 1, hits2);

      // 6. delete
      await invoke("memory_delete", { id: saved.id });
      log("memory_delete", true, null);

      // 7. recall after delete (should miss → verifies DELETE trigger)
      const hits3 = await invoke<MemoryRow[]>("memory_recall", {
        query: "updated",
      });
      log(
        `memory_recall after delete → ${hits3.length} hit(s) (DELETE trigger verified: ${hits3.length === 0})`,
        hits3.length === 0,
        hits3,
      );
    } catch (err) {
      log("ERROR", false, err);
    } finally {
      setRunning(false);
    }
  }

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
          Workspace selector, API key form, and provider picker will live here
          (Phase D + E).
        </p>
        <Button
          size="sm"
          variant="secondary"
          onClick={async () => {
            console.log("test---------");
            await invoke("get_workspace").catch((error) => {
              console.log(error);
            });
          }}
        >
          Coming soon
        </Button>
      </section>

      <section className="space-y-3 rounded-lg border border-border p-4">
        <div>
          <h3 className="font-medium">Memory layer smoke test</h3>
          <p className="text-sm text-muted-foreground">
            Runs save → recall → list → update → recall → delete → recall to
            verify the full Phase C3 pipeline (FTS5 INSERT/UPDATE/DELETE
            triggers included).
          </p>
        </div>
        <Button size="sm" onClick={runMemorySmokeTest} disabled={running}>
          {running ? "Running..." : "Run memory smoke test"}
        </Button>

        {steps.length > 0 && (
          <ol className="space-y-2 text-sm">
            {steps.map((s, i) => (
              <li
                key={i}
                className={`rounded border px-3 py-2 ${
                  s.ok
                    ? "border-green-500/40 bg-green-500/5"
                    : "border-red-500/40 bg-red-500/5"
                }`}
              >
                <div className="flex items-center gap-2 font-mono text-xs">
                  <span>{s.ok ? "✓" : "✗"}</span>
                  <span>{i + 1}. {s.label}</span>
                </div>
                <pre className="mt-1 max-h-40 overflow-auto rounded bg-muted/30 p-2 font-mono text-xs">
                  {JSON.stringify(s.data, null, 2)}
                </pre>
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}
