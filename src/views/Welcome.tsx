import { useEffect, useState } from "react";
import type { AppError } from "../bindings";
import { getAccountIdentity } from "../api/accounts";
import { setGlobalIdentity } from "../api/changes";
import { getGlobalIdentity } from "../api/settings";
import { InlineError } from "../components/InlineError";
import { SignInForm } from "../components/accounts/SignInForm";
import { useAccountStore } from "../stores/accountStore";
import { useSettingsStore } from "../stores/settingsStore";

/**
 * First run (spec §8.1): sign in to Forgejo or skip, then name and email for commits, saved
 * to the global git config only when the user clicks Save (CLAUDE.md rule 5).
 */
export function Welcome() {
  const accounts = useAccountStore((s) => s.accounts);
  const loaded = useAccountStore((s) => s.loaded);
  const load = useAccountStore((s) => s.load);
  const [skippedSignIn, setSkippedSignIn] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);

  const finish = () => void useSettingsStore.getState().update({ welcomeCompleted: true });
  const signedIn = accounts.length > 0;

  return (
    <main className="centered">
      <div className="welcome">
        <h1>Welcome to Tenajlo</h1>
        {!loaded ? null : !signedIn && !skippedSignIn ? (
          <>
            <p>
              Sign in to your Forgejo server to see your repositories and work with them without
              typing passwords. You can also do this later in Settings.
            </p>
            <SignInForm
              onSignedIn={() => void load()}
              onCancel={() => setSkippedSignIn(true)}
              cancelLabel="Skip"
            />
          </>
        ) : (
          <IdentityStep accountId={accounts[0]?.id ?? null} onDone={finish} />
        )}
      </div>
    </main>
  );
}

function IdentityStep({ accountId, onDone }: { accountId: string | null; onDone: () => void }) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  // Prefill: the global git config first, then the Forgejo profile for anything missing.
  useEffect(() => {
    let live = true;
    void (async () => {
      const global = await getGlobalIdentity();
      let n = global.status === "ok" ? (global.data.name ?? "") : "";
      let e = global.status === "ok" ? (global.data.email ?? "") : "";
      if (accountId && (!n || !e)) {
        const profile = await getAccountIdentity(accountId);
        if (profile.status === "ok") {
          n ||= profile.data.name ?? "";
          e ||= profile.data.email ?? "";
        }
      }
      if (!live) return;
      setName(n);
      setEmail(e);
      setLoaded(true);
    })();
    return () => {
      live = false;
    };
  }, [accountId]);

  const save = async () => {
    const res = await setGlobalIdentity(name, email);
    if (res.status === "error") return setError(res.error);
    onDone();
  };

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim() && email.trim()) void save();
      }}
    >
      <p>Git records your name and email on every commit you make.</p>
      <label>
        Name
        <input
          value={name}
          disabled={!loaded}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
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
      <p className="muted">
        Saving updates your global Git settings, so every repository on this computer uses them.
      </p>
      <InlineError error={error} />
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onDone}>
          Skip for now
        </button>
        <button type="submit" disabled={!loaded || !name.trim() || !email.trim()}>
          Save and continue
        </button>
      </div>
    </form>
  );
}
