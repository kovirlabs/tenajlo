import { useEffect, useState } from "react";
import type { AppError, RemoteRepository } from "../../bindings";
import { chooseCloneFolder, suggestClonePath } from "../../api/clone";
import { onProgress } from "../../api/sync";
import { useAccountStore } from "../../stores/accountStore";
import { useCloneStore } from "../../stores/cloneStore";
import { useRepoStore } from "../../stores/repoStore";
import { useUiStore } from "../../stores/uiStore";
import { InlineError } from "../InlineError";
import { Modal } from "../Modal";
import { ForgejoRepoList } from "./ForgejoRepoList";

type Tab = "forgejo" | "url";
type Protocol = "https" | "ssh";

/** Clone dialog (spec §8.1): pick a Forgejo repository or paste a URL, then a local folder. */
export function CloneDialog() {
  const open = useUiStore((s) => s.dialog === "clone");
  const close = () => {
    // Closing (e.g. Escape) while cloning stops the clone.
    useCloneStore.getState().cancel();
    // Also fires when another dialog replaced this one (e.g. "Sign in"); keep that one open.
    if (useUiStore.getState().dialog === "clone") useUiStore.getState().openDialog(null);
  };
  return (
    <Modal open={open} title="Clone a repository" onClose={close}>
      <CloneBody onClose={close} />
    </Modal>
  );
}

/** Mounted only while the dialog is open, so its state resets on close. */
function CloneBody({ onClose }: { onClose: () => void }) {
  const accounts = useAccountStore((s) => s.accounts);
  const loadAccounts = useAccountStore((s) => s.load);
  const running = useCloneStore((s) => s.running);
  const [tab, setTab] = useState<Tab>("forgejo");
  const [picked, setPicked] = useState<RemoteRepository | null>(null);
  const [protocol, setProtocol] = useState<Protocol>("https");
  const [typedUrl, setTypedUrl] = useState("");
  const [path, setPath] = useState("");
  const [pathChosen, setPathChosen] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const pickedUrl = picked ? (protocol === "ssh" ? picked.sshUrl : picked.cloneUrl) : "";
  const url = (tab === "forgejo" ? pickedUrl : typedUrl).trim();

  useEffect(() => {
    void loadAccounts();
  }, [loadAccounts]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void onProgress((p) => useCloneStore.getState().onProgress(p.opId, p.progress)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Suggest Documents/Tenajlo/<name> until the user picks a folder themselves.
  useEffect(() => {
    if (!url || pathChosen) return;
    let live = true;
    void suggestClonePath(url).then((res) => {
      if (live && res.status === "ok") setPath(res.data);
    });
    return () => {
      live = false;
    };
  }, [url, pathChosen]);

  const choose = async () => {
    const res = await chooseCloneFolder(url);
    if (res.status === "error") return setError(res.error);
    if (res.data) {
      setPath(res.data);
      setPathChosen(true);
    }
  };

  const clone = async () => {
    setError(null);
    const res = await useCloneStore.getState().clone(url, path.trim());
    if (res.status === "ok") {
      await useRepoStore.getState().load();
      onClose();
    } else if (res.error.kind !== "GitCancelled") {
      setError(res.error);
    }
  };

  return (
    <form
      className="form clone-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (url && path.trim() && !running) void clone();
      }}
    >
      <div role="tablist" className="tabs">
        {(
          [
            ["forgejo", "Your Forgejo repositories"],
            ["url", "URL"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className="tab"
            disabled={running !== null}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "forgejo" ? (
        <>
          <ForgejoRepoList
            accounts={accounts}
            selected={picked?.fullName ?? null}
            onSelect={setPicked}
          />
          <fieldset className="clone-protocol" disabled={running !== null}>
            <legend>Connect with</legend>
            <label>
              <input
                type="radio"
                name="protocol"
                checked={protocol === "https"}
                onChange={() => setProtocol("https")}
              />
              HTTPS (your Forgejo account)
            </label>
            <label>
              <input
                type="radio"
                name="protocol"
                checked={protocol === "ssh"}
                disabled={picked !== null && picked.sshUrl === ""}
                onChange={() => setProtocol("ssh")}
              />
              SSH (your SSH key)
            </label>
          </fieldset>
        </>
      ) : (
        <label>
          Repository address
          <input
            value={typedUrl}
            onChange={(e) => setTypedUrl(e.target.value)}
            placeholder="https://… or git@TMC-GIT01.tmus.local:team/project.git"
            spellCheck={false}
            autoFocus
          />
        </label>
      )}

      <label>
        Local folder
        <span className="clone-path">
          <input
            value={path}
            onChange={(e) => {
              setPath(e.target.value);
              setPathChosen(true);
            }}
            spellCheck={false}
          />
          <button type="button" className="secondary" disabled={!url} onClick={() => void choose()}>
            Choose…
          </button>
        </span>
      </label>

      <InlineError error={error} />

      {running ? (
        <div className="clone-progress" role="status" aria-live="polite">
          <span>
            {running.progress
              ? `${running.progress.phase}${running.progress.percent !== null ? ` ${running.progress.percent}%` : ""}`
              : "Starting…"}
          </span>
          <progress
            max={100}
            value={running.progress?.percent ?? undefined}
            aria-label="Clone progress"
          />
          <button
            type="button"
            className="secondary"
            onClick={() => useCloneStore.getState().cancel()}
          >
            Cancel
          </button>
        </div>
      ) : (
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" disabled={!url || !path.trim()}>
            Clone
          </button>
        </div>
      )}
    </form>
  );
}
