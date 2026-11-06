import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Commit } from "../../bindings";
import { getCommitChecks, openForgejoPage } from "../../api/forgejo";
import { useChangesStore } from "../../stores/changesStore";
import { useForgejoStore } from "../../stores/forgejoStore";
import { BranchPullRequestSection, PullRequestPill } from "./BranchPullRequest";
import { CommitChecksLine } from "./CommitChecksLine";

vi.mock("../../api/forgejo", () => ({
  getBranchLink: vi.fn(),
  getCommitChecks: vi.fn(),
  openForgejoPage: vi.fn(async () => ({ status: "ok", data: null })),
}));

const link = (pull: { number: number; title: string } | null) =>
  useForgejoStore.setState({
    repoId: "r",
    branch: "feature/x",
    link: { fullName: "team/pump", defaultBranch: "main", pull },
  });

const pushed = (upstream: string | null) =>
  useChangesStore.setState({
    repoId: "r",
    status: {
      branch: { name: "feature/x", tip: "a", upstream, ahead: 0, behind: 0, upstreamGone: false },
      files: [],
      hasConflicts: false,
    },
  });

const commit = (unpushed: boolean): Commit => ({
  sha: "a".repeat(40),
  shortSha: "aaaaaaa",
  parents: [],
  authorName: "Sam",
  authorEmail: "s@x",
  authorDate: "2026-11-05T10:00:00+00:00",
  summary: "Add alarm",
  body: "",
  unpushed,
});

describe("Forgejo links", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => {
    cleanup();
    useForgejoStore.setState({ repoId: null, branch: null, link: null });
  });

  it("shows the branch's pull request and opens it", () => {
    link({ number: 12, title: "Low pressure alarm" });
    pushed("origin/feature/x");
    const close = vi.fn();
    render(
      <>
        <PullRequestPill />
        <BranchPullRequestSection close={close} />
      </>,
    );
    expect(screen.getByTitle("Pull request #12: Low pressure alarm")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Low pressure alarm/ }));
    expect(close).toHaveBeenCalled();
    expect(openForgejoPage).toHaveBeenCalledWith("r", { type: "PullRequest", number: 12 });
  });

  it("offers Create pull request only for a pushed branch", () => {
    link(null);
    pushed(null);
    const { rerender } = render(<BranchPullRequestSection close={() => {}} />);
    expect(screen.queryByText("Create pull request…")).toBeNull();
    pushed("origin/feature/x");
    rerender(<BranchPullRequestSection close={() => {}} />);
    fireEvent.click(screen.getByText("Create pull request…"));
    expect(openForgejoPage).toHaveBeenCalledWith("r", {
      type: "NewPullRequest",
      branch: "feature/x",
    });
  });

  it("shows a pushed commit's checks and View on Forgejo", async () => {
    link(null);
    vi.mocked(getCommitChecks).mockResolvedValue({
      status: "ok",
      data: {
        state: "Failure",
        checks: [
          { context: "build (push)", state: "Success", description: "" },
          { context: "test (push)", state: "Failure", description: "Failing after 31s" },
        ],
      },
    });
    render(<CommitChecksLine repoId="r" commit={commit(false)} />);
    expect(await screen.findByText(/Checks failed \(1 of 2\)/)).toBeTruthy();
    fireEvent.click(screen.getByText("View on Forgejo"));
    expect(openForgejoPage).toHaveBeenCalledWith("r", { type: "Commit", sha: "a".repeat(40) });
  });

  it("hides links for commits that aren't pushed or repositories that aren't linked", () => {
    link(null);
    render(<CommitChecksLine repoId="r" commit={commit(true)} />);
    expect(screen.queryByText("View on Forgejo")).toBeNull();
    cleanup();
    useForgejoStore.setState({ link: null });
    render(<CommitChecksLine repoId="r" commit={commit(false)} />);
    expect(screen.queryByText("View on Forgejo")).toBeNull();
    expect(getCommitChecks).not.toHaveBeenCalled();
  });
});
