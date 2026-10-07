import { useEffect, useState } from "react";
import type { AuthPromptRequested } from "../bindings";
import { answerAuthPrompt, onAuthPrompt } from "../api/sync";
import { useCloneStore } from "../stores/cloneStore";
import { useSyncStore } from "../stores/syncStore";
import { Modal } from "./Modal";

/**
 * Sign-in prompts from git (via the askpass trampoline). One dialog covers git's separate
 * username and password prompts. Secrets live only in this component's state until sent.
 */
export function AuthPromptDialog() {
  const [queue, setQueue] = useState<AuthPromptRequested[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void onAuthPrompt((p) => setQueue((q) => [...q, p])).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Prompts for an operation that has finished or been cancelled are stale.
  const syncOp = useSyncStore((s) => s.running?.opId);
  const cloneOp = useCloneStore((s) => s.running?.opId);
  const isRunning = (opId: string) => opId === syncOp || opId === cloneOp;
  const current = queue.find((p) => isRunning(p.opId)) ?? null;
  const done = () => setQueue((q) => q.filter((p) => p !== current && isRunning(p.opId)));
  return current ? <PromptForm key={current.promptId} prompt={current} onDone={done} /> : null;
}

function PromptForm({ prompt, onDone }: { prompt: AuthPromptRequested; onDone: () => void }) {
  const { kind } = prompt;
  const [username, setUsername] = useState(kind.type === "Password" ? kind.username : "");
  const [secret, setSecret] = useState("");

  const finish = (send: boolean) => {
    const answer = send
      ? { username: kind.type === "Credentials" ? username : null, secret }
      : null;
    void answerAuthPrompt(prompt.promptId, answer);
    setSecret("");
    onDone();
  };

  const title = kind.type === "Other" ? "Git needs more information" : `Sign in to ${kind.host}`;
  const canSubmit = secret !== "" && (kind.type !== "Credentials" || username.trim() !== "");

  return (
    <Modal open title={title} onClose={() => finish(false)}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSubmit) finish(true);
        }}
      >
        {kind.type === "Other" ? (
          <p>
            <code>{kind.prompt}</code>
          </p>
        ) : (
          <p>Enter your username and password, or an access token instead of the password.</p>
        )}
        {kind.type === "Credentials" && (
          <label>
            Username
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
              autoFocus
            />
          </label>
        )}
        {kind.type === "Password" && (
          <p className="muted">
            Signing in as <strong>{kind.username}</strong>
          </p>
        )}
        <label>
          {kind.type === "Other" ? "Answer" : "Password or token"}
          <input
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            autoComplete="current-password"
            autoFocus={kind.type !== "Credentials"}
          />
        </label>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={() => finish(false)}>
            Cancel
          </button>
          <button type="submit" disabled={!canSubmit}>
            Sign in
          </button>
        </div>
      </form>
    </Modal>
  );
}
