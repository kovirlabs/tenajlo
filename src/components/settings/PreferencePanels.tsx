import { useState } from "react";
import type { AppError, Editor, EditorOption, PullStrategy, Theme } from "../../bindings";
import { chooseFile, chooseFolder, listEditors } from "../../api/settings";
import { useSettingsStore } from "../../stores/settingsStore";
import { InlineError } from "../InlineError";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";

const THEMES: { id: Theme; label: string }[] = [
  { id: "System", label: "Same as this computer" },
  { id: "Light", label: "Light" },
  { id: "Dark", label: "Dark" },
];

export function AppearancePanel() {
  const theme = useSettingsStore((s) => s.settings?.theme ?? "System");
  const update = useSettingsStore((s) => s.update);
  return (
    <fieldset className="settings-group">
      <legend>Theme</legend>
      {THEMES.map((t) => (
        <label key={t.id} className="radio">
          <input
            type="radio"
            name="theme"
            checked={theme === t.id}
            onChange={() => void update({ theme: t.id })}
          />
          {t.label}
        </label>
      ))}
    </fieldset>
  );
}

const PULL: { id: PullStrategy; label: string; help: string }[] = [
  {
    id: "FastForwardOnly",
    label: "Only when it's safe (recommended)",
    help: "Pull brings in the server's changes when you have no new commits of your own. If both changed, Tenajlo asks before merging.",
  },
  {
    id: "Merge",
    label: "Always merge",
    help: "If both changed, Pull merges the server's changes into yours with a merge commit.",
  },
  {
    id: "Rebase",
    label: "Rebase (advanced)",
    help: "Pull replays your commits on top of the server's. If that conflicts, Tenajlo can only undo it; finish it in a terminal.",
  },
];

const FETCH_MINUTES = [0, 1, 5, 15, 30, 60];

export function RepositoriesPanel() {
  const settings = useSettingsStore((s) => s.settings);
  const update = useSettingsStore((s) => s.update);
  const [error, setError] = useState<AppError | null>(null);
  if (!settings) return null;

  const save = async (change: Parameters<typeof update>[0]) => setError(await update(change));
  const choose = async () => {
    const folder = await chooseFolder("Choose where new clones go");
    if (folder) await save({ defaultCloneFolder: folder });
  };

  return (
    <div className="form">
      <fieldset className="settings-group">
        <legend>New clones go in</legend>
        <div className="clone-path">
          <input
            readOnly
            aria-label="Default clone folder"
            value={settings.defaultCloneFolder ?? "Documents/Tenajlo"}
          />
          <button type="button" className="secondary" onClick={() => void choose()}>
            Choose…
          </button>
          {settings.defaultCloneFolder && (
            <button
              type="button"
              className="secondary"
              onClick={() => void save({ defaultCloneFolder: null })}
            >
              Reset
            </button>
          )}
        </div>
      </fieldset>
      <fieldset className="settings-group">
        <legend>When pulling</legend>
        {PULL.map((p) => (
          <label key={p.id} className="radio">
            <input
              type="radio"
              name="pull"
              checked={settings.pullStrategy === p.id}
              onChange={() => void save({ pullStrategy: p.id })}
            />
            <span>
              {p.label}
              <span className="muted settings-help">{p.help}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <label>
        Check the server for changes
        <select
          value={settings.backgroundFetchMinutes}
          onChange={(e) => void save({ backgroundFetchMinutes: Number(e.target.value) })}
        >
          {FETCH_MINUTES.map((m) => (
            <option key={m} value={m}>
              {m === 0 ? "Never" : m === 1 ? "Every minute" : `Every ${m} minutes`}
            </option>
          ))}
        </select>
      </label>
      <EditorChoice onError={setError} />
      <InlineError error={error} />
    </div>
  );
}

/** "Open files in": detected editors, plus a custom program. */
function EditorChoice({ onError }: { onError: (e: AppError | null) => void }) {
  const editor = useSettingsStore((s) => s.settings?.editor ?? null);
  const update = useSettingsStore((s) => s.update);
  const [options, setOptions] = useState<EditorOption[]>([]);

  useAsyncEffect(async (live) => {
    const o = await listEditors();
    if (live()) setOptions(o);
  }, []);

  const save = async (next: Editor) => onError(await update({ editor: next }));
  const chooseCustom = async () => {
    const path = await chooseFile("Choose your editor program");
    if (path) await save({ kind: "Custom", path });
  };

  return (
    <label>
      Open files in
      <span className="clone-path">
        <select
          value={editor?.kind ?? "SystemDefault"}
          onChange={(e) => {
            const kind = e.target.value;
            if (kind === "Custom") void chooseCustom();
            else if (kind === "SystemDefault" || kind === "VsCode" || kind === "NotepadPlusPlus")
              void save({ kind });
          }}
        >
          {options.map((o) => (
            <option key={o.editor.kind} value={o.editor.kind} disabled={!o.available}>
              {o.label}
              {o.available ? "" : " (not installed)"}
            </option>
          ))}
          <option value="Custom">
            {editor?.kind === "Custom" ? `Other: ${editor.path}` : "Other program…"}
          </option>
        </select>
        {editor?.kind === "Custom" && (
          <button type="button" className="secondary" onClick={() => void chooseCustom()}>
            Change…
          </button>
        )}
      </span>
    </label>
  );
}
