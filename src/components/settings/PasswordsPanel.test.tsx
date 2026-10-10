import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { SavedSecretInfo } from "../../bindings";
import { PasswordsPanel } from "./PasswordsPanel";

const api = vi.hoisted(() => ({ listSavedSecrets: vi.fn(), forgetSavedSecret: vi.fn() }));
vi.mock("../../api/savedSecrets", () => api);

const login: SavedSecretInfo = {
  type: "Login",
  id: "login|https|git.example.com|evan",
  host: "git.example.com",
  username: "evan",
};
const passphrase: SavedSecretInfo = {
  type: "Passphrase",
  id: "ssh-passphrase|C:\\Users\\Evan G\\.ssh\\id_ed25519",
  key: "C:\\Users\\Evan G\\.ssh\\id_ed25519",
};

describe("PasswordsPanel", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("lists remembered secrets and forgets one", async () => {
    api.listSavedSecrets.mockResolvedValue([login, passphrase]);
    api.forgetSavedSecret.mockResolvedValue({ status: "ok", data: null });
    render(<PasswordsPanel />);

    expect(await screen.findByText("Password for git.example.com")).toBeTruthy();
    expect(screen.getByText("id_ed25519")).toBeTruthy();
    api.listSavedSecrets.mockResolvedValue([passphrase]);
    fireEvent.click(screen.getAllByRole("button", { name: "Forget" })[0] as HTMLElement);
    expect(api.forgetSavedSecret).toHaveBeenCalledWith(login.id);
    await waitFor(() => expect(screen.queryByText("Password for git.example.com")).toBeNull());
  });

  it("says when nothing is remembered", async () => {
    api.listSavedSecrets.mockResolvedValue([]);
    render(<PasswordsPanel />);
    expect(await screen.findByText("Nothing is remembered yet.")).toBeTruthy();
  });
});
