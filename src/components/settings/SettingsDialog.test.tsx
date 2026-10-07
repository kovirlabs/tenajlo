import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Settings } from "../../bindings";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { SettingsDialog } from "./SettingsDialog";

const defaults: Settings = {
  theme: "System",
  defaultCloneFolder: null,
  pullStrategy: "FastForwardOnly",
  backgroundFetchMinutes: 5,
  gitPath: null,
  editor: { kind: "SystemDefault" },
};

const api = vi.hoisted(() => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  chooseFolder: vi.fn(),
  chooseFile: vi.fn(),
  getGlobalIdentity: vi.fn(),
}));
vi.mock("../../api/settings", () => api);
vi.mock("../../api/changes", () => ({ setGlobalIdentity: vi.fn() }));
vi.mock("../../api/accounts", () => ({ listAccounts: vi.fn(async () => []) }));

function open(tab: "git" | "repositories" | "appearance") {
  useSettingsStore.setState({ settings: defaults });
  useUiStore.setState({ dialog: "settings", settingsTab: tab });
  render(<SettingsDialog />);
}

describe("SettingsDialog", () => {
  beforeEach(() => {
    api.saveSettings.mockImplementation(async (s: Settings) => ({ status: "ok", data: s }));
    api.getGlobalIdentity.mockResolvedValue({
      status: "ok",
      data: { name: "Evan", email: "e@x" },
    });
    delete document.documentElement.dataset.theme;
  });
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("applies the theme as soon as it's picked", async () => {
    open("appearance");
    fireEvent.click(screen.getByLabelText("Dark"));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    fireEvent.click(screen.getByLabelText("Same as this computer"));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBeUndefined());
  });

  it("saves pull strategy and shows rejected values inline", async () => {
    open("repositories");
    fireEvent.click(screen.getByLabelText(/Always merge/));
    await waitFor(() =>
      expect(api.saveSettings).toHaveBeenCalledWith({ ...defaults, pullStrategy: "Merge" }),
    );
    api.saveSettings.mockResolvedValueOnce({
      status: "error",
      error: {
        kind: "InvalidInput",
        gitKind: null,
        message: "Choose a full folder path for new clones.",
        details: null,
        accountId: null,
      },
    });
    api.chooseFolder.mockResolvedValue("relative");
    fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    expect((await screen.findByRole("alert")).textContent).toContain("full folder path");
  });

  it("switches git program and re-checks it", async () => {
    const recheckGit = vi.fn(async () => {});
    useAppStore.setState({ recheckGit });
    open("git");
    expect(await screen.findByDisplayValue("Evan")).toBeTruthy();
    api.chooseFile.mockResolvedValue("/opt/git/bin/git");
    fireEvent.click(screen.getByRole("button", { name: "Choose another Git…" }));
    await waitFor(() => expect(recheckGit).toHaveBeenCalled());
    expect(api.saveSettings).toHaveBeenCalledWith({ ...defaults, gitPath: "/opt/git/bin/git" });
  });
});
