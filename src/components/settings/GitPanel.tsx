import { useEffect, useState } from "react";
import type { AppError } from "../../bindings";
import { setGlobalIdentity } from "../../api/changes";
import { chooseFile, getGlobalIdentity } from "../../api/settings";
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
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    let live = true;
    void getGlobalIdentity().then((res) => {
      if (!live) return;
      if (res.status === "ok") {
        setName(res.data.name ?? "");
        setEmail(res.data.email ?? "");
      }
      setLoaded(true);
    });
    return () => {
      live = false;
    };
  }, []);

  const save = async () => {
    setSaved(false);
    const res = await setGlobalIdentity(name, email);
    if (res.status === "error") return setError(res.error);
    setError(null);
    setSaved(true);
  };

  return (
    <form
      className="settings-group form"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <h3>Your name and email</h3>
      <p className="muted">
        Git records these on every commit. Saving updates your global Git settings, so every
        repository on this computer uses them.
      </p>
      <label>
        Name
        <input value={name} disabled={!loaded} onChange={(e) => setName(e.target.value)} />
      </label>
      <label>
        Email
        <input
          type="email"
          value={email}
          disabled={!loaded}
          onChange={(e) => setEmail(e.target.value)}
        />
      </label>
      <InlineError error={error} />
      <div className="dialog-actions">
        {saved && <span className="muted">Saved</span>}
        <button type="submit" disabled={!loaded || !name.trim() || !email.trim()}>
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
            ? `Git LFS: ${info.lfsVersion.split(" ")[0]?.replace("git-lfs/", "")}`
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
