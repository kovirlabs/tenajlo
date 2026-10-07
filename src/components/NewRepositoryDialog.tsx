import { useEffect, useState } from "react";
import type { AppError } from "../bindings";
import { createRepository, defaultRepositoryFolder } from "../api/repos";
import { chooseFolder } from "../api/settings";
import { useRepoStore } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";
import { InlineError } from "./InlineError";
import { Modal } from "./Modal";

/** "New repository": a new folder with `git init` on branch main (spec §8.1). */
export function NewRepositoryDialog() {
  const open = useUiStore((s) => s.dialog === "newRepository");
  const close = () => {
    if (useUiStore.getState().dialog === "newRepository") useUiStore.getState().openDialog(null);
  };
  return (
    <Modal open={open} title="Create a new repository" onClose={close}>
      <NewRepositoryForm onClose={close} />
    </Modal>
  );
}

function NewRepositoryForm({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [parent, setParent] = useState("");
  const [readme, setReadme] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    let live = true;
    void defaultRepositoryFolder().then((res) => {
      if (live && res.status === "ok") setParent((current) => current || res.data);
    });
    return () => {
      live = false;
    };
  }, []);

  const choose = async () => {
    const folder = await chooseFolder("Choose where to create the repository");
    if (folder) setParent(folder);
  };

  const create = async () => {
    setBusy(true);
    setError(null);
    const res = await createRepository(parent, name, readme);
    setBusy(false);
    if (res.status === "error") return setError(res.error);
    await useRepoStore.getState().load();
    onClose();
  };

  const ready = name.trim() !== "" && parent.trim() !== "" && !busy;
  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) void create();
      }}
    >
      <label>
        Name
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. pump-station-controls"
          autoFocus
          spellCheck={false}
        />
      </label>
      <label>
        Location
        <span className="clone-path">
          <input value={parent} onChange={(e) => setParent(e.target.value)} spellCheck={false} />
          <button type="button" className="secondary" onClick={() => void choose()}>
            Choose…
          </button>
        </span>
      </label>
      <label className="radio">
        <input type="checkbox" checked={readme} onChange={(e) => setReadme(e.target.checked)} />
        Add a README file to describe the project
      </label>
      <p className="muted">
        Tenajlo creates a new folder named after the repository, ready for your first commit. To
        share it, create an empty repository on Forgejo and publish to it.
      </p>
      <InlineError error={error} />
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onClose}>
          Cancel
        </button>
        <button type="submit" disabled={!ready}>
          {busy ? "Creating…" : "Create repository"}
        </button>
      </div>
    </form>
  );
}
