import { useUiStore, type SettingsTab } from "../../stores/uiStore";
import { AccountsPanel } from "../accounts/AccountsPanel";
import { Modal } from "../Modal";
import { AboutPanel } from "./AboutPanel";
import { GitPanel } from "./GitPanel";
import { AppearancePanel, RepositoriesPanel } from "./PreferencePanels";
import { useDialog } from "../../hooks/useDialog";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "accounts", label: "Accounts" },
  { id: "git", label: "Git" },
  { id: "repositories", label: "Repositories" },
  { id: "appearance", label: "Appearance" },
  { id: "about", label: "About" },
];

/** Settings (spec §8.1). Every control saves as soon as it changes. */
export function SettingsDialog() {
  const { open, close } = useDialog("settings");
  const tab = useUiStore((s) => s.settingsTab);
  const setTab = useUiStore((s) => s.setSettingsTab);
  return (
    <Modal open={open} title="Settings" onClose={close}>
      <div className="settings">
        <div role="tablist" className="tabs">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className="tab"
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <div className="settings-panel" role="tabpanel">
          {tab === "accounts" && <AccountsPanel onClose={close} />}
          {tab === "git" && <GitPanel />}
          {tab === "repositories" && <RepositoriesPanel />}
          {tab === "appearance" && <AppearancePanel />}
          {tab === "about" && <AboutPanel />}
        </div>
        {tab !== "accounts" && (
          <div className="dialog-actions">
            <button type="button" onClick={close}>
              Done
            </button>
          </div>
        )}
      </div>
    </Modal>
  );
}
