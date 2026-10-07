import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Account, RemoteRepository } from "../../bindings";
import { useAccountStore } from "../../stores/accountStore";
import { useCloneStore } from "../../stores/cloneStore";
import { useUiStore } from "../../stores/uiStore";
import { CloneDialog } from "./CloneDialog";

const account: Account = {
  id: "a1",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "evan",
  displayName: "Evan",
  avatarUrl: null,
  needsSignIn: false,
};
const repo = (owner: string, name: string): RemoteRepository => ({
  fullName: `${owner}/${name}`,
  owner,
  name,
  description: `${name} description`,
  private: true,
  archived: false,
  cloneUrl: `https://git.example.com/${owner}/${name}.git`,
  sshUrl: `ssh://git@git.example.com:2222/${owner}/${name}.git`,
});

let accounts: Account[] = [];
const cloneApi = vi.hoisted(() => ({
  listForgejoRepositories: vi.fn(),
  suggestClonePath: vi.fn(),
  chooseCloneFolder: vi.fn(),
  cloneRepository: vi.fn(),
}));
const repoLoad = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("../../api/clone", () => cloneApi);
vi.mock("../../api/accounts", () => ({ listAccounts: vi.fn(async () => accounts) }));
vi.mock("../../api/sync", () => ({
  onProgress: vi.fn(async () => () => {}),
  cancelOperation: vi.fn(async () => {}),
}));
vi.mock("../../stores/repoStore", () => ({
  useRepoStore: { getState: () => ({ load: repoLoad }) },
}));

describe("CloneDialog", () => {
  beforeEach(() => {
    accounts = [account];
    useAccountStore.setState({ accounts: [], loaded: false });
    useCloneStore.setState({ lists: {}, loadingAccount: null, listError: null, running: null });
    useUiStore.setState({ dialog: "clone" });
    cloneApi.suggestClonePath.mockImplementation(async (url: string) => ({
      status: "ok",
      data: `/Users/evan/Documents/Tenajlo/${url.split("/").pop()?.replace(".git", "")}`,
    }));
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("clones a repository picked from the account's list", async () => {
    cloneApi.listForgejoRepositories.mockResolvedValue({
      status: "ok",
      data: [repo("evan", "notes"), repo("acme", "plc")],
    });
    cloneApi.cloneRepository.mockResolvedValue({ status: "ok", data: {} });
    render(<CloneDialog />);

    fireEvent.change(await screen.findByLabelText("Filter repositories"), {
      target: { value: "plc" },
    });
    expect(screen.queryByText("notes")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /plc/ }));

    const folder = screen.getByLabelText("Local folder") as HTMLInputElement;
    await waitFor(() => expect(folder.value).toBe("/Users/evan/Documents/Tenajlo/plc"));
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    await waitFor(() =>
      expect(cloneApi.cloneRepository).toHaveBeenCalledWith(
        expect.any(String),
        "https://git.example.com/acme/plc.git",
        "/Users/evan/Documents/Tenajlo/plc",
      ),
    );
    await waitFor(() => expect(useUiStore.getState().dialog).toBeNull());
    expect(repoLoad).toHaveBeenCalled();
  });

  it("clones over SSH when chosen", async () => {
    cloneApi.listForgejoRepositories.mockResolvedValue({
      status: "ok",
      data: [repo("acme", "plc")],
    });
    cloneApi.cloneRepository.mockResolvedValue({ status: "ok", data: {} });
    render(<CloneDialog />);
    fireEvent.click(await screen.findByRole("button", { name: /plc/ }));
    fireEvent.click(screen.getByLabelText("SSH (your SSH key)"));
    const folder = screen.getByLabelText("Local folder") as HTMLInputElement;
    await waitFor(() => expect(folder.value).toBe("/Users/evan/Documents/Tenajlo/plc"));
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    await waitFor(() =>
      expect(cloneApi.cloneRepository).toHaveBeenCalledWith(
        expect.any(String),
        "ssh://git@git.example.com:2222/acme/plc.git",
        "/Users/evan/Documents/Tenajlo/plc",
      ),
    );
  });

  it("clones a typed URL into a chosen folder and shows errors inline", async () => {
    cloneApi.listForgejoRepositories.mockResolvedValue({ status: "ok", data: [] });
    cloneApi.chooseCloneFolder.mockResolvedValue({ status: "ok", data: "/work/x" });
    cloneApi.cloneRepository.mockResolvedValue({
      status: "error",
      error: {
        kind: "Git",
        gitKind: "RepositoryNotFound",
        message: "The server couldn't find that repository.",
        details: "fatal: repository 'https://h/x.git/' not found",
        accountId: null,
      },
    });
    render(<CloneDialog />);
    fireEvent.click(screen.getByRole("tab", { name: "URL" }));
    fireEvent.change(screen.getByLabelText("Repository address"), {
      target: { value: "https://h/x.git" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    const folder = screen.getByLabelText("Local folder") as HTMLInputElement;
    await waitFor(() => expect(folder.value).toBe("/work/x"));

    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    expect((await screen.findByRole("alert")).textContent).toContain("couldn't find");
    expect(cloneApi.cloneRepository).toHaveBeenCalledWith(
      expect.any(String),
      "https://h/x.git",
      "/work/x",
    );
    expect(useUiStore.getState().dialog).toBe("clone");
  });

  it("asks to sign in when there are no accounts", async () => {
    accounts = [];
    render(<CloneDialog />);
    fireEvent.click(await screen.findByRole("button", { name: "Sign in to Forgejo…" }));
    expect(useUiStore.getState().dialog).toBe("settings");
  });

  it("offers to sign in again when the token stopped working", async () => {
    cloneApi.listForgejoRepositories.mockResolvedValue({
      status: "error",
      error: {
        kind: "SignInRequired",
        gitKind: null,
        message: "Your sign-in has stopped working.",
        details: null,
        accountId: "a1",
      },
    });
    render(<CloneDialog />);
    fireEvent.click(await screen.findByRole("button", { name: "Sign in again" }));
    expect(useUiStore.getState().signInAgainId).toBe("a1");
  });
});
