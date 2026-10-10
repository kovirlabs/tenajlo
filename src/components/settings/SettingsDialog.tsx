import { useUiStore, type SettingsTab } from "../../stores/uiStore";
import { AccountsPanel } from "../accounts/AccountsPanel";
import { Modal } from "../Modal";
import { AboutPanel } from "./AboutPanel";
import { GitPanel } from "./GitPanel";
import { SshKeysPanel } from "./SshKeysPanel";
import { PasswordsPanel } from "./PasswordsPanel";
import { AppearancePanel, RepositoriesPanel } from "./PreferencePanels";
import { useDialog } from "../../hooks/useDialog";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "accounts", label: "Accounts" },
  { id: "ssh", label: "SSH keys" },
  { id: "passwords", label: "Passwords" },
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
    <Modal open={open} title="Settings" onClose={close} className="settings-dialog">
      <div className="settings">
        {/* A list down the side: room for every section at any window width. */}
        <div role="tablist" aria-orientation="vertical" className="settings-nav">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className="settings-nav-item"
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <div className="settings-panel" role="tabpanel">
          {tab === "accounts" && <AccountsPanel onClose={close} />}
          {tab === "ssh" && <SshKeysPanel />}
          {tab === "passwords" && <PasswordsPanel />}
          {tab === "git" && <GitPanel />}
          {tab === "repositories" && <RepositoriesPanel />}
          {tab === "appearance" && <AppearancePanel />}
          {tab === "about" && <AboutPanel />}
        </div>
      </div>
      <div className="dialog-actions settings-actions">
        <button type="button" onClick={close}>
          Done
        </button>
      </div>
    </Modal>
  );
}
