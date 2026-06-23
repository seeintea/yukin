import { useState } from "react";
import { Button } from "#/components/ui/button";
import { tauri } from "#/utils/tauri";

interface SmokeStep {
  label: string;
  ok: boolean;
  data: unknown;
}

export function SettingsScreen() {
  const [memorySteps, setMemorySteps] = useState<SmokeStep[]>([]);
  const [memoryRunning, setMemoryRunning] = useState(false);

  const [keychainSteps, setKeychainSteps] = useState<SmokeStep[]>([]);
  const [keychainRunning, setKeychainRunning] = useState(false);

  const [sessionSteps, setSessionSteps] = useState<SmokeStep[]>([]);
  const [sessionRunning, setSessionRunning] = useState(false);

  const [workspaceSteps, setWorkspaceSteps] = useState<SmokeStep[]>([]);
  const [workspaceRunning, setWorkspaceRunning] = useState(false);

  const [fsSteps, setFsSteps] = useState<SmokeStep[]>([]);
  const [fsRunning, setFsRunning] = useState(false);

  async function runMemorySmokeTest() {
    setMemoryRunning(true);
    setMemorySteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setMemorySteps((p) => [...p, { label, ok, data }]);

    try {
      const saved = await tauri.memory.save({
        name: "smoke-test",
        kind: "user",
        content: "Hello memory layer!",
        metadata: { source: "settings-smoke-test" },
      });
      log("memory.save", true, saved);

      const hits1 = await tauri.memory.recall("Hello");
      log(`memory.recall("Hello") → ${hits1.length} hit(s)`, hits1.length === 1, hits1);

      const all = await tauri.memory.list();
      const inList = all.some((m) => m.id === saved.id);
      log(`memory.list → ${all.length} total, contains saved: ${inList}`, inList, all);

      const updated = await tauri.memory.update(saved.id, {
        content: "Hello updated content",
      });
      log(
        "memory.update content",
        updated.content === "Hello updated content",
        updated,
      );

      const hits2 = await tauri.memory.recall("updated");
      log(`memory.recall("updated") → ${hits2.length} hit(s)`, hits2.length === 1, hits2);

      await tauri.memory.delete(saved.id);
      log("memory.delete", true, null);

      const hits3 = await tauri.memory.recall("updated");
      log(
        `memory.recall after delete → ${hits3.length} hit(s) (DELETE trigger verified: ${hits3.length === 0})`,
        hits3.length === 0,
        hits3,
      );
    } catch (err) {
      log("ERROR", false, err);
    } finally {
      setMemoryRunning(false);
    }
  }

  async function runKeychainSmokeTest() {
    setKeychainRunning(true);
    setKeychainSteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setKeychainSteps((p) => [...p, { label, ok, data }]);

    try {
      await tauri.key.set("anthropic", "sk-ant-test-DELETE-ME");
      log("key.set anthropic", true, null);

      const list1 = await tauri.key.listProviders();
      log(
        `key.listProviders → [${list1.join(", ")}]`,
        list1.includes("anthropic"),
        list1,
      );

      const e1 = await tauri.key.exists("anthropic");
      const e2 = await tauri.key.exists("openai");
      log(
        `key.exists anthropic=${e1}, openai=${e2}`,
        e1 === true && e2 === false,
        { anthropic: e1, openai: e2 },
      );

      await tauri.key.delete("anthropic");
      log("key.delete anthropic", true, null);

      const list2 = await tauri.key.listProviders();
      const e3 = await tauri.key.exists("anthropic");
      log(
        `after delete: listProviders=[${list2.join(", ")}], exists=${e3}`,
        list2.length === 0 && e3 === false,
        { list: list2, exists: e3 },
      );
    } catch (err) {
      log("ERROR", false, err);
    } finally {
      setKeychainRunning(false);
    }
  }

  async function runSessionSmokeTest() {
    setSessionRunning(true);
    setSessionSteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setSessionSteps((p) => [...p, { label, ok, data }]);

    try {
      const s = await tauri.session.create({
        title: "Smoke test chat",
        provider: "anthropic",
        model: "claude-sonnet-4-6",
      });
      log("session.create", true, s);

      await tauri.session.appendMessage({
        sessionId: s.id,
        role: "user",
        content: JSON.stringify([{ type: "text", text: "Hi" }]),
      });
      await tauri.session.appendMessage({
        sessionId: s.id,
        role: "assistant",
        content: JSON.stringify([{ type: "text", text: "Hello" }]),
      });
      log("session.appendMessage ×2", true, null);

      const msgs = await tauri.session.loadMessages(s.id);
      log(`session.loadMessages → ${msgs.length} msgs`, msgs.length === 2, msgs);

      const updated = await tauri.session.update(s.id, { title: "Renamed" });
      log("session.update title → 'Renamed'", updated.title === "Renamed", updated);

      const all = await tauri.session.list();
      log(`session.list → ${all.length} total`, all.some((x) => x.id === s.id), all);

      await tauri.session.delete(s.id);
      const after = await tauri.session.loadMessages(s.id);
      log(
        `after delete: loadMessages → ${after.length} (cascade verified: ${after.length === 0})`,
        after.length === 0,
        after,
      );
    } catch (err) {
      log("ERROR", false, err);
    } finally {
      setSessionRunning(false);
    }
  }

  async function runWorkspaceTest() {
    setWorkspaceRunning(true);
    setWorkspaceSteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setWorkspaceSteps((p) => [...p, { label, ok, data }]);

    try {
      const current = await tauri.workspace.get();
      log(`workspace.get → ${current ?? "<unset>"}`, true, current);

      const picked = await tauri.workspace.select();
      log(`workspace.select → ${picked}`, true, picked);

      const reloaded = await tauri.workspace.get();
      log(
        `workspace.get after select → ${reloaded ?? "<unset>"}`,
        reloaded === picked,
        reloaded,
      );
    } catch (err) {
      log("ERROR (dialog cancelled is also expected here)", false, err);
    } finally {
      setWorkspaceRunning(false);
    }
  }

  async function runFsSmokeTest() {
    setFsRunning(true);
    setFsSteps([]);
    const log = (label: string, ok: boolean, data: unknown) =>
      setFsSteps((p) => [...p, { label, ok, data }]);

    try {
      const wsp = await tauri.workspace.get();
      log(`workspace.get → ${wsp ?? "<unset>"}`, !!wsp, wsp);
      if (!wsp) {
        log("workspace not set — run 'Pick workspace' first", false, null);
        return;
      }

      const testFile = "yukin-fs-smoke.txt";
      const testContent = "hello fs layer!";

      await tauri.fs.write(testFile, testContent);
      log(`fs.write ${testFile}`, true, null);

      const exists = await tauri.fs.exists(testFile);
      log(`fs.exists ${testFile} → ${exists}`, exists === true, exists);

      const read = await tauri.fs.read(testFile);
      log(
        `fs.read ${testFile} → ${read.content.length} bytes`,
        read.content === testContent,
        read,
      );

      await tauri.fs.edit(testFile, "fs layer", "fs LAYER");
      const edited = await tauri.fs.read(testFile);
      log(
        `fs.edit replace → ${edited.content}`,
        edited.content === "hello fs LAYER!",
        edited,
      );

      const dir = await tauri.fs.listDir(".");
      const found = dir.some((e) => e.name === testFile);
      log(`fs.listDir . → ${dir.length} entries, contains ${testFile}: ${found}`, found, dir);

      const globHits = await tauri.fs.glob("**/yukin-fs-smoke*");
      log(`fs.glob → ${globHits.length} match(es)`, globHits.length > 0, globHits);

      // Path safety check: traversal must be rejected
      try {
        await tauri.fs.read("../../etc/passwd");
        log("fs.read ../../etc/passwd UNEXPECTEDLY succeeded ⚠️", false, null);
      } catch (e) {
        log("fs.read ../../etc/passwd rejected (path_safety verified)", true, e);
      }
    } catch (err) {
      log("ERROR", false, err);
    } finally {
      setFsRunning(false);
    }
  }

  return (
    <div className="flex flex-1 flex-col gap-4 overflow-y-auto p-6">
      <header>
        <h2 className="text-2xl font-semibold">Settings</h2>
        <p className="text-sm text-muted-foreground">
          Smoke-test panel for all backend command groups. Each section runs the
          full IPC + Rust + storage chain end-to-end.
        </p>
      </header>

      <SmokeSection
        title="Workspace"
        description="get_workspace / select_workspace (opens native folder picker)."
        buttonLabel={workspaceRunning ? "Running..." : "Pick workspace"}
        onRun={runWorkspaceTest}
        running={workspaceRunning}
        steps={workspaceSteps}
      />

      <SmokeSection
        title="Filesystem"
        description="write → exists → read → edit → list_dir → glob → traversal rejection. Requires a workspace to be picked first."
        buttonLabel={fsRunning ? "Running..." : "Run fs smoke test"}
        onRun={runFsSmokeTest}
        running={fsRunning}
        steps={fsSteps}
      />

      <SmokeSection
        title="Memory layer"
        description="save → recall → list → update → recall → delete → recall (verifies FTS5 INSERT/UPDATE/DELETE triggers)."
        buttonLabel={memoryRunning ? "Running..." : "Run memory smoke test"}
        onRun={runMemorySmokeTest}
        running={memoryRunning}
        steps={memorySteps}
      />

      <SmokeSection
        title="Keychain"
        description="key_set → list_providers → key_exists → key_delete (verifies OS Keychain integration; check Keychain Access for xyz.yukin.agent)."
        buttonLabel={keychainRunning ? "Running..." : "Run keychain smoke test"}
        onRun={runKeychainSmokeTest}
        running={keychainRunning}
        steps={keychainSteps}
      />

      <SmokeSection
        title="Sessions"
        description="create → append × 2 → load → update → list → delete (verifies ON DELETE CASCADE for messages)."
        buttonLabel={sessionRunning ? "Running..." : "Run session smoke test"}
        onRun={runSessionSmokeTest}
        running={sessionRunning}
        steps={sessionSteps}
      />
    </div>
  );
}

interface SmokeSectionProps {
  title: string;
  description: string;
  buttonLabel: string;
  onRun: () => void;
  running: boolean;
  steps: SmokeStep[];
}

function SmokeSection({
  title,
  description,
  buttonLabel,
  onRun,
  running,
  steps,
}: SmokeSectionProps) {
  return (
    <section className="space-y-3 rounded-lg border border-border p-4">
      <div>
        <h3 className="font-medium">{title}</h3>
        <p className="text-sm text-muted-foreground">{description}</p>
      </div>
      <Button size="sm" onClick={onRun} disabled={running}>
        {buttonLabel}
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
                <span>
                  {i + 1}. {s.label}
                </span>
              </div>
              <pre className="mt-1 max-h-40 overflow-auto rounded bg-muted/30 p-2 font-mono text-xs">
                {JSON.stringify(s.data, null, 2)}
              </pre>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
