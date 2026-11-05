import mark from "../../src-tauri/app-icon.svg";
import { useSelectedRepository } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";
import { BranchDropdown } from "./BranchDropdown";
import { RepoDropdown } from "./RepoDropdown";
import { SyncButton } from "./SyncButton";

export function Toolbar() {
  const repo = useSelectedRepository();
  return (
    <header className="toolbar">
      <img className="toolbar-mark" src={mark} alt="" aria-hidden="true" />
      <RepoDropdown />
      {repo && !repo.missing && (
        <>
          <BranchDropdown />
          <SyncButton />
        </>
      )}
      <span className="toolbar-spacer" />
      <button
        type="button"
        className="secondary toolbar-settings"
        title="Settings (Ctrl+,)"
        onClick={() => useUiStore.getState().openSettings("accounts")}
      >
        Settings
      </button>
    </header>
  );
}
