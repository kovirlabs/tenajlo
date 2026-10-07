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
  if (!current) return null;
  return current.kind.type === "HostKey" ? (
    <HostKeyForm key={current.promptId} prompt={current} onDone={done} />
  ) : (
    <PromptForm key={current.promptId} prompt={current} onDone={done} />
  );
}

/**
 * OpenSSH hasn't seen this server before. Accepting lets OpenSSH record the key in
 * known_hosts itself; cancelling stops the operation. Never accepted automatically.
 */
function HostKeyForm({ prompt, onDone }: { prompt: AuthPromptRequested; onDone: () => void }) {
  const { kind } = prompt;
  if (kind.type !== "HostKey") return null;
  const finish = (trust: boolean) => {
    // Rust turns any answer into OpenSSH's "yes"; null cancels.
    void answerAuthPrompt(prompt.promptId, trust ? { username: null, secret: "" } : null);
    onDone();
  };
  return (
    <Modal open title={`Connect to ${kind.host}?`} onClose={() => finish(false)}>
      <div className="form">
        <p>
          Tenajlo hasn't connected to this server from this computer before. To make sure you're
          talking to the real server, check with your IT team that its fingerprint is:
        </p>
        <p className="fingerprint">
          <span className="muted">{kind.key_type}</span> <code>{kind.fingerprint}</code>
        </p>
        <p className="muted">If it doesn't match, cancel and tell your IT team.</p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={() => finish(false)} autoFocus>
            Cancel
          </button>
          <button type="button" onClick={() => finish(true)}>
            Trust and connect
          </button>
        </div>
      </div>
    </Modal>
  );
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

  const title =
    kind.type === "Other"
      ? "Git needs more information"
      : kind.type === "Passphrase"
        ? "Unlock your SSH key"
        : `Sign in to ${kind.host}`;
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
        ) : kind.type === "Passphrase" ? (
          <>
            <p>
              Enter the passphrase for your SSH key <code>{kind.key}</code>.
            </p>
            {kind.retry && (
              <p className="form-error" role="alert">
                That passphrase didn't work. Try again.
              </p>
            )}
          </>
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
          {kind.type === "Other"
            ? "Answer"
            : kind.type === "Passphrase"
              ? "Passphrase"
              : "Password or token"}
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
            {kind.type === "Passphrase" ? "Unlock" : "Sign in"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
