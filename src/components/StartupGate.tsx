import { useEffect, type ReactNode } from "react";
import type { GitVersion } from "../bindings";
import { useAppStore } from "../stores/appStore";
import { useSettingsStore } from "../stores/settingsStore";
import { ErrorDetails } from "./ErrorDetails";

const formatVersion = (v: GitVersion) => `${v.major}.${v.minor}.${v.patch}`;

/** Blocks the app until a supported git is found (spec §5.1). */
export function StartupGate({ children }: { children: ReactNode }) {
  const gitCheck = useAppStore((s) => s.gitCheck);
  const runGitCheck = useAppStore((s) => s.runGitCheck);

  const loadSettings = useSettingsStore((s) => s.load);

  useEffect(() => {
    // Settings first: they may name a different git program.
    void loadSettings().then(runGitCheck);
  }, [loadSettings, runGitCheck]);

  switch (gitCheck.phase) {
    case "checking":
      return (
        <main className="centered" aria-busy="true">
          <p>Checking for Git…</p>
        </main>
      );
    case "ready":
      return <>{children}</>;
    case "unsupported":
      return (
        <Blocker
          title="Git needs an update"
          message={`Tenajlo needs Git ${formatVersion(gitCheck.info.minimum)} or newer. This computer has Git ${formatVersion(gitCheck.info.version)}. Update Git, then try again.`}
          details={`Git found at: ${gitCheck.info.path}`}
          onRetry={runGitCheck}
        />
      );
    case "failed":
      return (
        <Blocker
          title="Git isn't available"
          message={gitCheck.error.message}
          details={gitCheck.error.details}
          onRetry={runGitCheck}
        />
      );
  }
}

type BlockerProps = {
  title: string;
  message: string;
  details: string | null;
  onRetry: () => void;
};

function Blocker({ title, message, details, onRetry }: BlockerProps) {
  return (
    <main className="centered">
      <div className="blocker" role="alert">
        <h1>{title}</h1>
        <p>{message}</p>
        <ErrorDetails details={details} />
        <button type="button" onClick={onRetry}>
          Try again
        </button>
        <UseDefaultGit onDone={onRetry} />
      </div>
    </main>
  );
}

/** Escape hatch when Settings points at a git program that doesn't work. */
function UseDefaultGit({ onDone }: { onDone: () => void }) {
  const gitPath = useSettingsStore((s) => s.settings?.gitPath ?? null);
  const update = useSettingsStore((s) => s.update);
  if (!gitPath) return null;
  return (
    <button
      type="button"
      className="secondary"
      onClick={() => void update({ gitPath: null }).then(onDone)}
    >
      Use the default Git
    </button>
  );
}
