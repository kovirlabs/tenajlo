import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Account, PublishOptions } from "../bindings";
import { useAccountStore } from "../stores/accountStore";
import { useRepoStore } from "../stores/repoStore";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";
import { PublishRepositoryDialog } from "./PublishRepositoryDialog";

const api = vi.hoisted(() => ({ getPublishOptions: vi.fn(), publishRepository: vi.fn() }));
vi.mock("../api/publish", () => api);
const accountsApi = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  checkServer: vi.fn(),
  signIn: vi.fn(),
  signOut: vi.fn(),
  openTokenSettings: vi.fn(),
}));
vi.mock("../api/accounts", () => accountsApi);
const refreshRepo = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("../stores/refreshRepo", () => ({ refreshRepo, registerRefresh: vi.fn() }));

const account: Account = {
  id: "acc-1",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "evan",
  displayName: "Evan",
  avatarUrl: null,
  needsSignIn: false,
};

const options = (o: Partial<PublishOptions> = {}): PublishOptions => ({
  accountId: account.id,
  login: "evan",
  userAllowed: true,
  organizations: ["controls"],
  organizationsNeedToken: false,
  requiredScopes: ["write:user", "write:organization"],
  ...o,
});

describe("PublishRepositoryDialog", () => {
  const run = vi.fn(async () => {});
  beforeEach(() => {
    useAccountStore.setState({ accounts: [account], loaded: true, load: async () => {} });
    useRepoStore.setState({
      repositories: [
        { id: "repo-1", name: "pump-station", path: "/w/pump-station", missing: false },
      ],
      selectedId: "repo-1",
    });
    useSyncStore.setState({ run });
    useUiStore.setState({ dialog: "publishRepository" });
  });
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("creates the repository in an organization, then publishes the branch", async () => {
    api.getPublishOptions.mockResolvedValue({ status: "ok", data: options() });
    api.publishRepository.mockResolvedValue({ status: "ok", data: {} });
    render(<PublishRepositoryDialog />);

    expect(await screen.findByDisplayValue("pump-station")).toBeTruthy();
    expect(screen.getByRole("option", { name: "evan (you)" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Owner"), { target: { value: "controls" } });
    fireEvent.click(screen.getByLabelText(/SSH/));
    fireEvent.click(screen.getByRole("button", { name: "Publish repository" }));

    await waitFor(() =>
      expect(api.publishRepository).toHaveBeenCalledWith({
        repoId: "repo-1",
        accountId: "acc-1",
        organization: "controls",
        name: "pump-station",
        description: "",
        isPrivate: true,
        ssh: true,
      }),
    );
    await waitFor(() => expect(run).toHaveBeenCalledWith("Publish"));
    expect(refreshRepo).toHaveBeenCalledWith("repo-1");
    expect(useUiStore.getState().dialog).toBeNull();
  });

  it("shows server refusals inline and stays open", async () => {
    api.getPublishOptions.mockResolvedValue({ status: "ok", data: options() });
    api.publishRepository.mockResolvedValue({
      status: "error",
      error: {
        kind: "InvalidInput",
        gitKind: null,
        message: "There's already a repository named pump-station in your account.",
        details: null,
        accountId: null,
      },
    });
    render(<PublishRepositoryDialog />);
    fireEvent.click(await screen.findByRole("button", { name: "Publish repository" }));
    expect(await screen.findByText(/already a repository named pump-station/)).toBeTruthy();
    expect(run).not.toHaveBeenCalled();
    expect(useUiStore.getState().dialog).toBe("publishRepository");
  });

  it("offers a new token when the saved one can't create repositories", async () => {
    api.getPublishOptions.mockResolvedValue({
      status: "ok",
      data: options({ userAllowed: false, organizations: [], organizationsNeedToken: true }),
    });
    accountsApi.checkServer.mockResolvedValue({
      status: "ok",
      data: {
        baseUrl: account.baseUrl,
        version: "11.0.0",
        tokenSettingsUrl: `${account.baseUrl}/user/settings/applications`,
        requiredScopes: ["read:user"],
      },
    });
    render(<PublishRepositoryDialog />);
    fireEvent.click(await screen.findByRole("button", { name: "Update the token" }));
    expect(await screen.findByText(/that can create repositories/)).toBeTruthy();
    await waitFor(() => expect(accountsApi.checkServer).toHaveBeenCalledWith(account.baseUrl));
    expect(await screen.findByText("write:organization")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Publish repository" })).toBeNull();
  });

  it("asks to sign in when there is no account", async () => {
    useAccountStore.setState({ accounts: [] });
    render(<PublishRepositoryDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Sign in…" }));
    expect(useUiStore.getState()).toMatchObject({ dialog: "settings", settingsTab: "accounts" });
    expect(api.getPublishOptions).not.toHaveBeenCalled();
  });
});
