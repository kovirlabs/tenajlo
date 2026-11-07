import { useEffect } from "react";
import { SettingsDialog } from "../components/settings/SettingsDialog";
import { AuthPromptDialog } from "../components/AuthPromptDialog";
import { CloneDialog } from "../components/clone/CloneDialog";
import { NewRepositoryDialog } from "../components/NewRepositoryDialog";
import { PublishRepositoryDialog } from "../components/PublishRepositoryDialog";
import { ErrorDialog } from "../components/ErrorDialog";
import { NewBranchDialog } from "../components/branches/NewBranchDialog";
import { LocksDialog } from "../components/locks/LocksDialog";
import { useCloneLinks } from "../hooks/useCloneLinks";
import { useShortcuts } from "../hooks/useShortcuts";
import { useCloneStore } from "../stores/cloneStore";
import { useUiStore } from "../stores/uiStore";
import { Toolbar } from "../components/Toolbar";
import { UpdateBanner } from "../components/UpdateBanner";
import { useUpdateStore } from "../stores/updateStore";
import { useRepoStore, useSelectedRepository } from "../stores/repoStore";
import { MissingRepository } from "./MissingRepository";
import { NoRepository } from "./NoRepository";
import { useSettingsStore } from "../stores/settingsStore";
import { Welcome } from "./Welcome";
import { RepositoryView } from "./RepositoryView";

export function AppShell() {
  const loaded = useRepoStore((s) => s.loaded);
  const load = useRepoStore((s) => s.load);
  const repo = useSelectedRepository();
  const repoCount = useRepoStore((s) => s.repositories.length);
  const welcomeCompleted = useSettingsStore((s) => s.settings?.welcomeCompleted ?? true);
  const checkForUpdates = useSettingsStore((s) => s.settings?.checkForUpdates ?? false);

  // Once per start, if the user allows it. Builds that can't update themselves just say no.
  useEffect(() => {
    if (checkForUpdates) void useUpdateStore.getState().check(true);
  }, [checkForUpdates]);

  // People who already have repositories (e.g. from an earlier version) skip the welcome.
  useEffect(() => {
    if (loaded && repoCount > 0 && !welcomeCompleted) {
      void useSettingsStore.getState().update({ welcomeCompleted: true });
    }
  }, [loaded, repoCount, welcomeCompleted]);

  useEffect(() => {
    void load();
  }, [load]);

  useShortcuts(repo !== null && !repo.missing);

  const showWelcome = !repo && repoCount === 0 && !welcomeCompleted;
  const dialog = useUiStore((s) => s.dialog);
  const cloning = useCloneStore((s) => s.running !== null);
  // Open in Tenajlo links wait until nothing else is in the way (spec §8.3).
  useCloneLinks(loaded && !showWelcome && (dialog === null || dialog === "clone") && !cloning);

  let body;
  if (!loaded) body = <main className="centered" aria-busy="true" />;
  else if (showWelcome) body = <Welcome />;
  else if (!repo) body = <NoRepository />;
  else if (repo.missing) body = <MissingRepository repo={repo} />;
  else body = <RepositoryView key={repo.id} repo={repo} />;

  return (
    <div className="shell">
      <Toolbar />
      <UpdateBanner />
      {body}
      {/* Not `key={repo.id}`: RepositoryView is a sibling with that key, and React
          duplicates siblings that share a key (old repositories stay on screen). */}
      {repo && !repo.missing && <NewBranchDialog key={`new-branch:${repo.id}`} />}
      {repo && !repo.missing && <LocksDialog key={`locks:${repo.id}`} repoId={repo.id} />}
      <CloneDialog />
      <NewRepositoryDialog />
      <PublishRepositoryDialog />
      <SettingsDialog />
      <AuthPromptDialog />
      <ErrorDialog />
    </div>
  );
}
