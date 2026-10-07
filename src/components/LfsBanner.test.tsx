import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { LfsStatus } from "../bindings";
import { LfsBanner } from "./LfsBanner";

const getLfsStatus = vi.fn<(id: string) => Promise<{ status: "ok"; data: LfsStatus }>>();
vi.mock("../api/status", () => ({ getLfsStatus: (id: string) => getLfsStatus(id) }));

describe("LfsBanner", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("warns when the repository needs LFS and it's missing", async () => {
    getLfsStatus.mockResolvedValue({ status: "ok", data: { used: true, installed: false } });
    render(<LfsBanner repoId="r" />);
    expect((await screen.findByRole("alert")).textContent).toContain("Git LFS isn't installed");
  });

  it("stays hidden when LFS is installed or unused", async () => {
    getLfsStatus.mockResolvedValue({ status: "ok", data: { used: true, installed: true } });
    render(<LfsBanner repoId="r" />);
    await vi.waitFor(() => expect(getLfsStatus).toHaveBeenCalledWith("r"));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
