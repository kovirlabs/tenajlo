import { useEffect } from "react";
import { SettingsDialog } from "../components/settings/SettingsDialog";
import { AuthPromptDialog } from "../components/AuthPromptDialog";
import { CloneDialog } from "../components/clone/CloneDialog";
import { NewRepositoryDialog } from "../components/NewRepositoryDialog";
import { ErrorDialog } from "../components/ErrorDialog";
import { NewBranchDialog } from "../components/branches/NewBranchDialog";
import { useShortcuts } from "../hooks/useShortcuts";
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

  let body;
  if (!loaded) body = <main className="centered" aria-busy="true" />;
  else if (!repo && repoCount === 0 && !welcomeCompleted) body = <Welcome />;
  else if (!repo) body = <NoRepository />;
  else if (repo.missing) body = <MissingRepository repo={repo} />;
  else body = <RepositoryView key={repo.id} repo={repo} />;

  return (
    <div className="shell">
      <Toolbar />
      <UpdateBanner />
      {body}
      {repo && !repo.missing && <NewBranchDialog key={repo.id} />}
      <CloneDialog />
      <NewRepositoryDialog />
      <SettingsDialog />
      <AuthPromptDialog />
      <ErrorDialog />
    </div>
  );
}
