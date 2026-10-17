import { afterEach, describe, expect, it } from "vitest";
import { act, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Repository, WorkingDirectoryStatus } from "../bindings";
import { useRepoStore } from "../stores/repoStore";
import { AppShell } from "./AppShell";

const repos: Repository[] = ["alpha", "beta", "gamma"].map((name, i) => ({
  id: `00000000-0000-4000-8000-00000000000${i}`,
  name,
  path: `/repos/${name}`,
  missing: false,
}));

const status: WorkingDirectoryStatus = {
  branch: { name: "main", tip: null, upstream: null, ahead: 0, behind: 0, upstreamGone: false },
  files: [],
  hasConflicts: false,
};

describe("AppShell", () => {
  afterEach(() => clearMocks());

  it("shows only the selected repository after switching", async () => {
    let selectedId = repos[0]?.id ?? null;
    mockIPC(
      (cmd, args) => {
        if (cmd === "list_repositories") return { repositories: repos, selectedId };
        if (cmd === "select_repository") selectedId = (args as { id: string }).id;
        if (cmd === "get_status") return status;
        if (cmd === "get_lfs_status") return { used: false, installed: true };
        if (cmd === "get_history") return [];
        return null;
      },
      { shouldMockEvents: true },
    );

    const { container, unmount } = render(<AppShell />);
    await waitFor(() => expect(container.querySelectorAll(".repo-view")).toHaveLength(1));

    for (const repo of [...repos.slice(1), ...repos]) {
      await act(() => useRepoStore.getState().select(repo.id));
      expect(container.querySelectorAll(".repo-view")).toHaveLength(1);
      expect(container.querySelectorAll(".sidebar")).toHaveLength(1);
    }
    unmount();
  });
});
