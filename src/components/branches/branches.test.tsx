import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Branch, FileChange } from "../../bindings";
import { useBranchStore } from "../../stores/branchStore";
import { useChangesStore } from "../../stores/changesStore";
import { useUiStore } from "../../stores/uiStore";
import { BranchDropdown } from "../BranchDropdown";
import { NewBranchDialog } from "./NewBranchDialog";

const api = {
  switchBranch: vi.fn(async () => ({ status: "ok" })),
  deleteBranch: vi.fn(),
  createBranch: vi.fn(async () => ({ status: "ok" })),
  previewBranchName: vi.fn(async (n: string) => n.trim().replace(/\s+/g, "-")),
};
vi.mock("../../api/branches", () => ({
  switchBranch: (...a: unknown[]) => (api.switchBranch as (...x: unknown[]) => unknown)(...a),
  deleteBranch: (...a: unknown[]) => (api.deleteBranch as (...x: unknown[]) => unknown)(...a),
  createBranch: (...a: unknown[]) => (api.createBranch as (...x: unknown[]) => unknown)(...a),
  previewBranchName: (n: string) => api.previewBranchName(n),
  getSavedChanges: vi.fn(),
  getBranches: vi.fn(),
}));

const branch = (name: string, over: Partial<Branch> = {}): Branch => ({
  name,
  kind: "Local",
  remote: null,
  upstream: null,
  tip: "abc",
  isCurrent: false,
  lastCommitDate: "",
  ...over,
});

function setup(files: FileChange[]) {
  // Like the real one: runs the operation and resolves to its result.
  const mutate = vi.fn((op: (id: string) => Promise<{ status: string }>) => op("r1"));
  useChangesStore.setState({
    repoId: "r1",
    busy: false,
    mutate: mutate as never,
    status: {
      branch: { name: "main", tip: "x", upstream: null, ahead: 0, behind: 0, upstreamGone: false },
      files,
      hasConflicts: false,
    },
  });
  useBranchStore.setState({
    branches: {
      current: "main",
      local: [branch("main", { isCurrent: true }), branch("feature")],
      remoteOnly: [branch("origin/hotfix", { kind: "Remote", remote: "origin" })],
    },
  });
  render(<BranchDropdown />);
  fireEvent.click(screen.getByRole("button", { name: "Current branch" }));
}

const change: FileChange = {
  path: "a.txt",
  oldPath: null,
  kind: "Modified",
  staged: "None",
  submodule: false,
};

describe("branch actions", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("switches directly with a clean working directory", async () => {
    setup([]);
    fireEvent.click(screen.getByRole("button", { name: "origin/hotfix" }));
    await waitFor(() =>
      expect(api.switchBranch).toHaveBeenCalledWith("r1", "origin/hotfix", "Bring"),
    );
  });

  it("asks what to do with local changes, defaulting to leave them", async () => {
    setup([change]);
    fireEvent.click(screen.getByRole("button", { name: "feature" }));
    expect(api.switchBranch).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Switch branch" }));
    await waitFor(() => expect(api.switchBranch).toHaveBeenCalledWith("r1", "feature", "Leave"));
  });

  it("asks again before force-deleting an unmerged branch", async () => {
    api.deleteBranch
      .mockResolvedValueOnce({
        status: "error",
        error: { kind: "Git", gitKind: "BranchNotMerged", message: "", details: null },
      })
      .mockResolvedValueOnce({ status: "ok" });
    setup([]);
    fireEvent.contextMenu(screen.getByRole("button", { name: "feature" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await screen.findByText(/aren't on any other branch/);
    expect(api.deleteBranch).toHaveBeenLastCalledWith("r1", "feature", false);
    fireEvent.click(screen.getByRole("button", { name: "Delete anyway" }));
    await waitFor(() => expect(api.deleteBranch).toHaveBeenLastCalledWith("r1", "feature", true));
  });
});

describe("NewBranchDialog", () => {
  it("shows the sanitized name and creates it", async () => {
    setup([]);
    cleanup();
    useUiStore.setState({ dialog: "newBranch" });
    render(<NewBranchDialog />);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "fix pump alarm" } });
    expect(await screen.findByText("fix-pump-alarm")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Create branch" }));
    await waitFor(() => expect(api.createBranch).toHaveBeenCalledWith("r1", "fix-pump-alarm"));
  });

  it("creates the name as typed even if Enter beats the preview", async () => {
    setup([]);
    cleanup();
    useUiStore.setState({ dialog: "newBranch" });
    render(<NewBranchDialog />);
    const input = screen.getByLabelText("Name");
    fireEvent.change(input, { target: { value: "fix" } });
    expect(await screen.findByRole("button", { name: "Create branch" })).toHaveProperty(
      "disabled",
      false,
    );
    fireEvent.change(input, { target: { value: "fix pump" } });
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    await waitFor(() => expect(api.createBranch).toHaveBeenLastCalledWith("r1", "fix-pump"));
  });
});
