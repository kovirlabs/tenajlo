import { useAppStore } from "../stores/appStore";

/** Placeholder main window until repositories arrive in M1. */
export function EmptyShell() {
  const gitCheck = useAppStore((s) => s.gitCheck);
  const v = gitCheck.phase === "ready" ? gitCheck.info.version : null;

  return (
    <div className="shell">
      <header className="toolbar">
        <span className="toolbar-item">No repository</span>
      </header>
      <main className="centered">
        <div className="empty">
          <h1>Welcome to Anvil</h1>
          <p>Adding and cloning repositories is coming next.</p>
          {v && (
            <p className="muted">
              Using Git {v.major}.{v.minor}.{v.patch}
            </p>
          )}
        </div>
      </main>
    </div>
  );
}
