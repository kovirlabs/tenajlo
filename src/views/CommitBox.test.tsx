import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { FileChange, Identity } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { CommitBox } from "./CommitBox";

const identity = vi.fn<() => Promise<{ status: "ok"; data: Identity }>>();
const commitChanges = vi.fn(async () => ({ status: "ok" as const, data: "abc" }));
vi.mock("../api/changes", () => ({
  getIdentity: () => identity(),
  commitChanges: (...a: unknown[]) => (commitChanges as (...x: unknown[]) => unknown)(...a),
  setGlobalIdentity: vi.fn(),
}));

const file = (staged: FileChange["staged"]): FileChange => ({
  path: "a.txt",
  oldPath: null,
  kind: "Modified",
  staged,
  submodule: false,
});

function setup(staged: FileChange["staged"], id: Identity = { name: "E", email: "e@x" }) {
  identity.mockResolvedValue({ status: "ok", data: id });
  const mutate = vi.fn(async (op: (id: string) => Promise<unknown>) => {
    await op("r1");
    return true;
  });
  useChangesStore.setState({
    repoId: "r1",
    busy: false,
    mutate: mutate as never,
    status: {
      branch: { name: "main", tip: "x", upstream: null, ahead: 0, behind: 0, upstreamGone: false },
      files: [file(staged)],
      hasConflicts: false,
    },
  });
  render(<CommitBox repoId="r1" />);
  return mutate;
}

describe("CommitBox", () => {
  beforeEach(() => commitChanges.mockClear());
  afterEach(cleanup);

  it("commits with Ctrl+Enter and clears the message", async () => {
    setup("Full");
    const summary = screen.getByLabelText("Commit summary") as HTMLInputElement;
    fireEvent.change(summary, { target: { value: "Fix pump" } });
    await waitFor(() => expect(identity).toHaveBeenCalled());
    fireEvent.keyDown(summary, { key: "Enter", ctrlKey: true });
    await waitFor(() => expect(commitChanges).toHaveBeenCalledWith("r1", "Fix pump", ""));
    await waitFor(() => expect(summary.value).toBe(""));
  });

  it("is disabled with nothing staged or no summary", () => {
    setup("None");
    fireEvent.change(screen.getByLabelText("Commit summary"), { target: { value: "x" } });
    const button = screen.getByRole("button", { name: /Commit to main/ }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("asks for an identity instead of committing when none is set", async () => {
    setup("Full", { name: null, email: null });
    expect(await screen.findByText(/needs your name and email/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Commit summary"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /Commit to main/ }));
    expect(commitChanges).not.toHaveBeenCalled();
    expect(screen.getByText(/global Git settings/)).toBeTruthy();
  });
});
