import { useEffect, useState } from "react";
import type { Identity } from "../bindings";
import { getAccountIdentity } from "../api/accounts";
import { getGlobalIdentity } from "../api/settings";
import { IdentityFields } from "../components/IdentityFields";
import { InlineError } from "../components/InlineError";
import { useIdentityForm } from "../hooks/useIdentityForm";
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
  // Prefill: the global git config first, then the Forgejo profile for anything missing.
  const form = useIdentityForm(async () => {
    const global = await getGlobalIdentity();
    const known: Partial<Identity> = global.status === "ok" ? global.data : {};
    if (!accountId || (known.name && known.email)) return known;
    const profile = await getAccountIdentity(accountId);
    if (profile.status !== "ok") return known;
    return {
      name: known.name || profile.data.name,
      email: known.email || profile.data.email,
    };
  });

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (form.valid) void form.save().then((ok) => ok && onDone());
      }}
    >
      <p>Git records your name and email on every commit you make.</p>
      <IdentityFields form={form} autoFocus />
      <p className="muted">
        Saving updates your global Git settings, so every repository on this computer uses them.
      </p>
      <InlineError error={form.error} />
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onDone}>
          Skip for now
        </button>
        <button type="submit" disabled={!form.loaded || !form.valid || form.saving}>
          Save and continue
        </button>
      </div>
    </form>
  );
}
