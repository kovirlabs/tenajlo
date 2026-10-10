import { useState } from "react";
import type { Account, AppError, ServerInfo } from "../../bindings";
import { checkServer, openTokenSettings, signIn } from "../../api/accounts";
import { InlineError } from "../InlineError";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";

type Props = {
  /** Known server (signing in again): skips straight to checking it. */
  initialServer?: string;
  onSignedIn: (account: Account) => void;
  onCancel: () => void;
  /** Label of the first step's cancel button (the welcome screen says "Skip"). */
  cancelLabel?: string;
};

/** Two-step Forgejo sign-in (spec §6.4): server address, then a personal access token. */
export function SignInForm({ initialServer, onSignedIn, onCancel, cancelLabel }: Props) {
  const [server, setServer] = useState(initialServer ?? "");
  const [info, setInfo] = useState<ServerInfo | null>(null);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(Boolean(initialServer));
  const [error, setError] = useState<AppError | null>(null);

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    await work();
    setBusy(false);
  };

  const check = () =>
    run(async () => {
      const res = await checkServer(server);
      if (res.status === "error") return setError(res.error);
      setInfo(res.data);
    });

  useAsyncEffect(
    async (live) => {
      if (!initialServer) return;
      const res = await checkServer(initialServer);
      if (!live()) return;
      setBusy(false);
      if (res.status === "error") setError(res.error);
      else setInfo(res.data);
    },
    [initialServer],
  );

  const submit = (srv: ServerInfo) =>
    run(async () => {
      const res = await signIn(srv.baseUrl, token);
      if (res.status === "error") return setError(res.error);
      onSignedIn(res.data);
    });

  const errorBox = <InlineError error={error} />;

  if (!info) {
    return (
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (server.trim()) void check();
        }}
      >
        <label>
          Server address
          <input
            value={server}
            onChange={(e) => setServer(e.target.value)}
            placeholder="e.g. git.example.com"
            autoFocus
            spellCheck={false}
          />
        </label>
        {errorBox}
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            {cancelLabel ?? "Cancel"}
          </button>
          <button type="submit" disabled={!server.trim() || busy}>
            {busy ? "Checking…" : "Continue"}
          </button>
        </div>
      </form>
    );
  }

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (token.trim()) void submit(info);
      }}
    >
      <p>
        Create an access token on <strong>{info.baseUrl}</strong>, then paste it below.{" "}
        <button
          type="button"
          className="link"
          onClick={() => {
            void openTokenSettings(info.baseUrl).then((r) => {
              if (r.status === "error") setError(r.error);
            });
          }}
        >
          Open token settings in your browser
        </button>
      </p>
      <p className="muted">
        Give the token these permissions:{" "}
        {info.requiredScopes.map((s, i) => (
          <span key={s}>
            {i > 0 && ", "}
            <code>{s}</code>
          </span>
        ))}
        . Tenajlo keeps it in this computer's password store, never in a file.
      </p>
      <label>
        Access token
        <input
          type="password"
          value={token}
          onChange={(e) => setToken(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          autoFocus
        />
      </label>
      {errorBox}
      <div className="dialog-actions">
        <button
          type="button"
          className="secondary"
          onClick={() => {
            setInfo(null);
            setToken("");
            setError(null);
          }}
        >
          Back
        </button>
        <button type="submit" disabled={!token.trim() || busy}>
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </div>
    </form>
  );
}
