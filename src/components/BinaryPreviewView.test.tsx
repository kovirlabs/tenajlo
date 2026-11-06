import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { BinaryPreview } from "../bindings";
import { useLockStore } from "../stores/lockStore";
import { BinaryPreviewView } from "./BinaryPreviewView";

const PNG = "data:image/png;base64,iVBORw0KGgo=";

function show(preview: BinaryPreview, lfs = false, path = "img/logo.png") {
  render(
    <BinaryPreviewView
      lfs={lfs}
      preview={{ key: path, path, fetch: async () => ({ status: "ok", data: preview }) }}
    />,
  );
}

describe("BinaryPreviewView", () => {
  afterEach(() => {
    cleanup();
    useLockStore.setState({ available: false, locks: [] });
  });

  it("shows both sizes and the images side by side", async () => {
    show({
      old: { size: 4096, lfs: false, image: PNG },
      new: { size: 6144, lfs: false, image: null },
    });
    expect(await screen.findByText(/4\.0 KB → 6\.0 KB/)).toBeTruthy();
    expect(screen.getByText("(+2.0 KB)")).toBeTruthy();
    expect(screen.getByAltText("Before version").getAttribute("src")).toBe(PNG);
    expect(screen.getByText("No preview")).toBeTruthy();
  });

  it("marks LFS files, their lock, and content that isn't downloaded", async () => {
    useLockStore.setState({
      repoId: "r",
      available: true,
      locks: [{ id: "1", path: "cad/pump.SLDPRT", owner: "alice", lockedAt: "", ours: false }],
    });
    show({ old: null, new: { size: 3_000_000, lfs: true, image: null } }, true, "cad/pump.SLDPRT");
    expect(await screen.findByText("New file · 2.9 MB")).toBeTruthy();
    expect(screen.getByText("Stored with Git LFS")).toBeTruthy();
    expect(screen.getByText(/Locked by alice/)).toBeTruthy();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("says when a file was deleted", async () => {
    show({ old: { size: 812, lfs: false, image: null }, new: null });
    expect(await screen.findByText("Deleted · was 812 bytes")).toBeTruthy();
  });

  it("reloads when the working tree changes", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce({
        status: "ok",
        data: { old: null, new: { size: 10, lfs: false, image: null } },
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: { old: null, new: { size: 20, lfs: false, image: null } },
      });
    const props = (version: number) => ({ key: "k", version, path: "a.png", fetch });
    const { rerender } = render(<BinaryPreviewView lfs={false} preview={props(1)} />);
    expect(await screen.findByText("New file · 10 bytes")).toBeTruthy();
    rerender(<BinaryPreviewView lfs={false} preview={props(2)} />);
    expect(await screen.findByText("New file · 20 bytes")).toBeTruthy();
  });
});
