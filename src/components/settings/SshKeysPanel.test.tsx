import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Account, AccountSshKeys, LocalSshKey } from "../../bindings";
import { useAccountStore } from "../../stores/accountStore";
import { SshKeysPanel } from "./SshKeysPanel";

const account: Account = {
  id: "a1",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "evan",
  displayName: "Evan G",
  avatarUrl: null,
  needsSignIn: false,
};
const key: LocalSshKey = {
  fileName: "id_ed25519.pub",
  keyType: "ssh-ed25519",
  comment: "Work laptop",
  fingerprint: "SHA256:abc",
};
const access = (over: Partial<AccountSshKeys>): AccountSshKeys => ({
  accountId: "a1",
  canAdd: true,
  fingerprints: [],
  requiredScopes: ["write:user", "read:repository", "write:repository"],
  ...over,
});

const ssh = vi.hoisted(() => ({
  listSshKeys: vi.fn(),
  createSshKey: vi.fn(),
  getAccountSshKeys: vi.fn(),
  addSshKey: vi.fn(),
}));
vi.mock("../../api/sshKeys", () => ssh);
const accountsApi = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  checkServer: vi.fn(),
  signIn: vi.fn(),
  signOut: vi.fn(),
  openTokenSettings: vi.fn(),
}));
vi.mock("../../api/accounts", () => accountsApi);

describe("SshKeysPanel", () => {
  beforeEach(() => {
    accountsApi.listAccounts.mockResolvedValue([account]);
    useAccountStore.setState({ accounts: [], loaded: false });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("adds a key once the token is allowed to", async () => {
    ssh.listSshKeys.mockResolvedValue({ status: "ok", data: [key] });
    ssh.getAccountSshKeys.mockResolvedValue({ status: "ok", data: access({}) });
    ssh.addSshKey.mockResolvedValue({ status: "ok", data: null });
    render(<SshKeysPanel />);

    fireEvent.click(await screen.findByRole("button", { name: "Add to git.example.com" }));
    expect(ssh.addSshKey).toHaveBeenCalledWith("a1", "id_ed25519.pub", "Work laptop");
    // Reloads afterwards; the key now shows as added.
    ssh.getAccountSshKeys.mockResolvedValue({
      status: "ok",
      data: access({ fingerprints: ["SHA256:abc"] }),
    });
    expect(await screen.findByText("Added to git.example.com")).toBeTruthy();
  });

  it("offers a new token instead of Add when the token lacks permission", async () => {
    ssh.listSshKeys.mockResolvedValue({ status: "ok", data: [key] });
    ssh.getAccountSshKeys.mockResolvedValue({ status: "ok", data: access({ canAdd: false }) });
    accountsApi.checkServer.mockResolvedValue({
      status: "ok",
      data: {
        baseUrl: "https://git.example.com",
        version: "11",
        tokenSettingsUrl: "https://git.example.com/user/settings/applications",
        requiredScopes: ["read:user", "read:repository", "write:repository"],
      },
    });
    render(<SshKeysPanel />);

    fireEvent.click(await screen.findByRole("button", { name: "Update the token" }));
    expect(screen.queryByRole("button", { name: /Add to/ })).toBeNull();
    expect(await screen.findByText("write:user")).toBeTruthy();
    expect(accountsApi.checkServer).toHaveBeenCalledWith("https://git.example.com");
  });

  it("creates a key with a passphrase", async () => {
    ssh.listSshKeys.mockResolvedValue({ status: "ok", data: [] });
    ssh.getAccountSshKeys.mockResolvedValue({ status: "ok", data: access({}) });
    ssh.createSshKey.mockResolvedValue({ status: "ok", data: key });
    render(<SshKeysPanel />);

    fireEvent.click(await screen.findByRole("button", { name: "Create an SSH key…" }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Work laptop" } });
    fireEvent.change(screen.getByLabelText("Passphrase (optional)"), { target: { value: "pw" } });
    fireEvent.change(screen.getByLabelText("Confirm passphrase"), { target: { value: "px" } });
    expect(screen.getByText("The passphrases don't match.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create key" })).toHaveProperty("disabled", true);

    fireEvent.change(screen.getByLabelText("Confirm passphrase"), { target: { value: "pw" } });
    fireEvent.click(screen.getByRole("button", { name: "Create key" }));
    await waitFor(() => expect(ssh.createSshKey).toHaveBeenCalledWith("Work laptop", "pw"));
  });

  it("hides Create when id_ed25519 already exists", async () => {
    ssh.listSshKeys.mockResolvedValue({ status: "ok", data: [key] });
    ssh.getAccountSshKeys.mockResolvedValue({ status: "ok", data: access({}) });
    render(<SshKeysPanel />);
    await screen.findByText("id_ed25519.pub");
    expect(screen.queryByRole("button", { name: "Create an SSH key…" })).toBeNull();
  });
});
