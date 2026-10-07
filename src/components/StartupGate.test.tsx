import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { AppError, GitInfo, Settings } from "../bindings";
import type { Result } from "../api/result";
import { useAppStore } from "../stores/appStore";
import { StartupGate } from "./StartupGate";

const checkGit = vi.fn<() => Promise<Result<GitInfo>>>();
vi.mock("../api/git", () => ({ checkGit: () => checkGit() }));

const defaults: Settings = {
  theme: "System",
  defaultCloneFolder: null,
  pullStrategy: "FastForwardOnly",
  backgroundFetchMinutes: 5,
  gitPath: null,
  editor: { kind: "SystemDefault" },
};
let stored: Settings = defaults;
const saveSettings = vi.fn(async (s: Settings) => {
  stored = s;
  return { status: "ok" as const, data: s };
});
vi.mock("../api/settings", () => ({
  getSettings: vi.fn(async () => stored),
  saveSettings: (s: Settings) => saveSettings(s),
}));

const info = (minor: number): GitInfo => ({
  path: "/usr/bin/git",
  version: { major: 2, minor, patch: 1 },
  minimum: { major: 2, minor: 40, patch: 0 },
  supported: minor >= 40,
  lfsVersion: "git-lfs/3.7.1 (GitHub; darwin arm64)",
});

const renderGate = () =>
  render(
    <StartupGate>
      <p>app content</p>
    </StartupGate>,
  );

describe("StartupGate", () => {
  beforeEach(() => {
    stored = defaults;
    useAppStore.setState({ gitCheck: { phase: "checking" } });
  });
  afterEach(() => {
    cleanup();
    checkGit.mockReset();
  });

  it("renders children when git is supported", async () => {
    checkGit.mockResolvedValue({ status: "ok", data: info(45) });
    renderGate();
    expect(await screen.findByText("app content")).toBeTruthy();
  });

  it("blocks with a plain-language message when git is too old", async () => {
    checkGit.mockResolvedValue({ status: "ok", data: info(39) });
    renderGate();
    expect(await screen.findByText("Git needs an update")).toBeTruthy();
    expect(screen.getByText(/needs Git 2\.40\.0 or newer.*has Git 2\.39\.1/)).toBeTruthy();
    expect(screen.queryByText("app content")).toBeNull();
  });

  it("shows the backend message when git is missing and retries", async () => {
    const error: AppError = {
      kind: "GitNotFound",
      gitKind: null,
      accountId: null,
      message: "Tenajlo couldn't find Git on this computer.",
      details: "Looked in: /usr/bin/git",
    };
    checkGit.mockResolvedValueOnce({ status: "error", error });
    renderGate();
    expect(await screen.findByText(error.message)).toBeTruthy();

    checkGit.mockResolvedValueOnce({ status: "ok", data: info(45) });
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("app content")).toBeTruthy();
  });

  it("offers the default Git when a chosen git program doesn't work", async () => {
    stored = { ...defaults, gitPath: "/opt/broken/git" };
    const error: AppError = {
      kind: "GitNotFound",
      gitKind: null,
      message: "Tenajlo couldn't find Git.",
      details: null,
      accountId: null,
    };
    checkGit.mockResolvedValueOnce({ status: "error", error });
    checkGit.mockResolvedValueOnce({ status: "ok", data: info(45) });
    renderGate();
    fireEvent.click(await screen.findByRole("button", { name: "Use the default Git" }));
    expect(await screen.findByText("app content")).toBeTruthy();
    expect(saveSettings).toHaveBeenCalledWith({ ...defaults, gitPath: null });
  });
});
