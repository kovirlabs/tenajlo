import { useState } from "react";
import type { AppError } from "../../bindings";
import { chooseFile, getGlobalIdentity } from "../../api/settings";
import { useIdentityForm } from "../../hooks/useIdentityForm";
import { IdentityFields } from "../IdentityFields";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { InlineError } from "../InlineError";

/** Git identity (global config, after explicit Save) and which git program to use. */
export function GitPanel() {
  return (
    <div className="form">
      <IdentityForm />
      <GitLocation />
    </div>
  );
}

function IdentityForm() {
  const form = useIdentityForm(async () => {
    const res = await getGlobalIdentity();
    return res.status === "ok" ? res.data : {};
  });
  const [saved, setSaved] = useState(false);

  return (
    <form
      className="settings-group form"
      onSubmit={(e) => {
        e.preventDefault();
        setSaved(false);
        void form.save().then(setSaved);
      }}
    >
      <h3>Your name and email</h3>
      <p className="muted">
        Git records these on every commit. Saving updates your global Git settings, so every
        repository on this computer uses them.
      </p>
      <IdentityFields form={form} />
      <InlineError error={form.error} />
      <div className="dialog-actions">
        {saved && <span className="muted">Saved</span>}
        <button type="submit" disabled={!form.loaded || !form.valid || form.saving}>
          Save name and email
        </button>
      </div>
    </form>
  );
}

function GitLocation() {
  const gitPath = useSettingsStore((s) => s.settings?.gitPath ?? null);
  const update = useSettingsStore((s) => s.update);
  const check = useAppStore((s) => s.gitCheck);
  const recheck = useAppStore((s) => s.recheckGit);
  const [error, setError] = useState<AppError | null>(null);

  const apply = async (path: string | null) => {
    const err = await update({ gitPath: path });
    setError(err);
    if (!err) await recheck();
  };
  const choose = async () => {
    const path = await chooseFile("Choose the git program");
    if (path) await apply(path);
  };

  const info = check.phase === "ready" ? check.info : null;
  return (
    <div className="settings-group">
      <h3>Git program</h3>
      {info && (
        <p className="muted">
          Using Git {info.version.major}.{info.version.minor}.{info.version.patch} at{" "}
          <code>{info.path}</code>
          {gitPath ? " (chosen in Settings)" : ""}
        </p>
      )}
      {info && (
        <p className="muted">
          {info.lfsVersion
            ? `Git LFS: ${info.lfsVersion}`
            : "Git LFS isn't installed. Repositories with large files (CAD, PLC archives) need it: get it from git-lfs.com."}
        </p>
      )}
      <InlineError error={error} />
      <div className="dialog-actions">
        {gitPath && (
          <button type="button" className="secondary" onClick={() => void apply(null)}>
            Use the default Git
          </button>
        )}
        <button type="button" className="secondary" onClick={() => void choose()}>
          Choose another Git…
        </button>
      </div>
    </div>
  );
}
