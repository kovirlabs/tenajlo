import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Account } from "../bindings";
import { useAccountStore } from "../stores/accountStore";
import { useSettingsStore } from "../stores/settingsStore";
import { Welcome } from "./Welcome";

const account: Account = {
  id: "a1",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "evan",
  displayName: "Evan G",
  avatarUrl: null,
  needsSignIn: false,
};
let accounts: Account[] = [];
const api = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  getAccountIdentity: vi.fn(),
  checkServer: vi.fn(),
  signIn: vi.fn(),
  openTokenSettings: vi.fn(),
}));
const getGlobalIdentity = vi.hoisted(() => vi.fn());
const setGlobalIdentity = vi.hoisted(() => vi.fn());
vi.mock("../api/accounts", () => api);
vi.mock("../api/settings", () => ({ getGlobalIdentity }));
vi.mock("../api/changes", () => ({ setGlobalIdentity }));

describe("Welcome", () => {
  const update = vi.fn(async () => null);
  beforeEach(() => {
    accounts = [];
    api.listAccounts.mockImplementation(async () => accounts);
    getGlobalIdentity.mockResolvedValue({ status: "ok", data: { name: null, email: null } });
    setGlobalIdentity.mockResolvedValue({ status: "ok", data: null });
    useAccountStore.setState({ accounts: [], loaded: false });
    useSettingsStore.setState({ update });
  });
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("can skip both steps", async () => {
    render(<Welcome />);
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip for now" }));
    expect(update).toHaveBeenCalledWith({ welcomeCompleted: true });
    expect(setGlobalIdentity).not.toHaveBeenCalled();
  });

  it("prefills name and email from the Forgejo profile and saves only on request", async () => {
    accounts = [account];
    api.getAccountIdentity.mockResolvedValue({
      status: "ok",
      data: { name: "Evan G", email: "evan@example.com" },
    });
    render(<Welcome />);
    const name = (await screen.findByLabelText("Name")) as HTMLInputElement;
    await waitFor(() => expect(name.value).toBe("Evan G"));
    expect((screen.getByLabelText("Email") as HTMLInputElement).value).toBe("evan@example.com");
    expect(api.getAccountIdentity).toHaveBeenCalledWith("a1");
    expect(setGlobalIdentity).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save and continue" }));
    await waitFor(() =>
      expect(setGlobalIdentity).toHaveBeenCalledWith("Evan G", "evan@example.com"),
    );
    await waitFor(() => expect(update).toHaveBeenCalledWith({ welcomeCompleted: true }));
  });

  it("keeps an existing global identity", async () => {
    accounts = [account];
    getGlobalIdentity.mockResolvedValue({
      status: "ok",
      data: { name: "Evan", email: "e@x.org" },
    });
    render(<Welcome />);
    const email = (await screen.findByLabelText("Email")) as HTMLInputElement;
    await waitFor(() => expect(email.value).toBe("e@x.org"));
    expect(api.getAccountIdentity).not.toHaveBeenCalled();
  });
});
