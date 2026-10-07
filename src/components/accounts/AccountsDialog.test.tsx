import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Account, ServerInfo } from "../../bindings";
import { useAccountStore } from "../../stores/accountStore";
import { useUiStore } from "../../stores/uiStore";
import { AccountsDialog } from "./AccountsDialog";

const account: Account = {
  id: "a1",
  kind: "forgejo",
  baseUrl: "https://tmc-git01.tmus.local",
  login: "evan",
  displayName: "Evan G",
  avatarUrl: null,
  needsSignIn: false,
};
const info: ServerInfo = {
  baseUrl: "https://tmc-git01.tmus.local",
  version: "11.0.1",
  tokenSettingsUrl: "https://tmc-git01.tmus.local/user/settings/applications",
  requiredScopes: ["read:user", "read:repository", "write:repository"],
};

let stored: Account[] = [];
const api = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  checkServer: vi.fn(),
  signIn: vi.fn(),
  signOut: vi.fn(),
  openTokenSettings: vi.fn(),
}));
vi.mock("../../api/accounts", () => api);

describe("AccountsDialog", () => {
  beforeEach(() => {
    stored = [];
    api.listAccounts.mockImplementation(async () => stored);
    useAccountStore.setState({ accounts: [], loaded: false });
    useUiStore.setState({ dialog: "accounts", signInAgainId: null });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("signs in with server then token", async () => {
    api.checkServer.mockResolvedValue({ status: "ok", data: info });
    api.signIn.mockImplementation(async () => {
      stored = [account];
      return { status: "ok", data: account };
    });
    render(<AccountsDialog />);

    fireEvent.change(await screen.findByLabelText("Server address"), {
      target: { value: "TMC-GIT01.tmus.local" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(api.checkServer).toHaveBeenCalledWith("TMC-GIT01.tmus.local");

    fireEvent.change(await screen.findByLabelText("Access token"), {
      target: { value: "pat-123" },
    });
    expect(screen.getByText("write:repository")).toBeTruthy();
    api.openTokenSettings.mockResolvedValue({ status: "ok", data: null });
    fireEvent.click(screen.getByRole("button", { name: "Open token settings in your browser" }));
    expect(api.openTokenSettings).toHaveBeenCalledWith(info.baseUrl);

    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(api.signIn).toHaveBeenCalledWith(info.baseUrl, "pat-123");
    expect(await screen.findByText("Evan G")).toBeTruthy();
    expect(screen.getByText("(evan)")).toBeTruthy();
  });

  it("shows server errors inline with details", async () => {
    api.checkServer.mockResolvedValue({
      status: "error",
      error: {
        kind: "Server",
        gitKind: null,
        accountId: null,
        message: "Couldn't reach the server.",
        details: "dns error",
      },
    });
    render(<AccountsDialog />);
    fireEvent.change(await screen.findByLabelText("Server address"), {
      target: { value: "nope" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Couldn't reach the server.");
    expect(screen.getByText("dns error")).toBeTruthy();
  });

  it("asks before signing out", async () => {
    stored = [account];
    api.signOut.mockImplementation(async () => {
      stored = [];
      return { status: "ok", data: null };
    });
    render(<AccountsDialog />);
    fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
    expect(api.signOut).not.toHaveBeenCalled();
    expect(screen.getByText(/saved token will be removed/)).toBeTruthy();
    // The row's button is replaced by the confirmation, so only one "Sign out" remains.
    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
    await waitFor(() => expect(api.signOut).toHaveBeenCalledWith("a1"));
    expect(await screen.findByLabelText("Server address")).toBeTruthy();
  });

  it("goes straight to a new token when signing in again", async () => {
    stored = [{ ...account, needsSignIn: true }];
    api.checkServer.mockResolvedValue({ status: "ok", data: info });
    api.signIn.mockImplementation(async () => {
      stored = [account];
      return { status: "ok", data: account };
    });
    useUiStore.setState({ dialog: "accounts", signInAgainId: "a1" });
    render(<AccountsDialog />);

    expect(await screen.findByText(/has stopped working/)).toBeTruthy();
    expect(api.checkServer).toHaveBeenCalledWith(account.baseUrl);
    fireEvent.change(await screen.findByLabelText("Access token"), {
      target: { value: "new-pat" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(api.signIn).toHaveBeenCalledWith(info.baseUrl, "new-pat");
    expect(await screen.findByText("Evan G")).toBeTruthy();
    expect(screen.queryByText("Sign-in has stopped working")).toBeNull();
  });

  it("flags accounts whose token stopped working", async () => {
    stored = [{ ...account, needsSignIn: true }];
    api.checkServer.mockResolvedValue({ status: "ok", data: info });
    render(<AccountsDialog />);
    expect(await screen.findByText("Sign-in has stopped working")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    expect(await screen.findByLabelText("Access token")).toBeTruthy();
  });
});
