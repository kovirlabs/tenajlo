import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render } from "@testing-library/react";
import { useCloneStore } from "../stores/cloneStore";
import { useUiStore } from "../stores/uiStore";
import { useCloneLinks } from "./useCloneLinks";

const link = vi.hoisted(() => ({
  pending: null as string | null,
  fire: null as (() => void) | null,
}));
vi.mock("../api/openLink", () => ({
  takeCloneLink: vi.fn(async () => {
    const url = link.pending;
    link.pending = null;
    return url;
  }),
  onCloneLink: vi.fn(async (cb: () => void) => {
    link.fire = cb;
    return () => {
      link.fire = null;
    };
  }),
}));

function Probe({ ready }: { ready: boolean }) {
  useCloneLinks(ready);
  return null;
}

const flush = () => act(async () => {});

describe("useCloneLinks", () => {
  beforeEach(() => {
    link.pending = null;
    useUiStore.setState({ dialog: null });
    useCloneStore.setState({ linkUrl: null, running: null });
  });
  afterEach(() => vi.clearAllMocks());

  it("opens the Clone dialog for a link that started Tenajlo once the app is ready", async () => {
    link.pending = "https://git.example.com/acme/plc.git";
    const { rerender } = render(<Probe ready={false} />);
    await flush();
    expect(useUiStore.getState().dialog).toBeNull();
    expect(link.pending).not.toBeNull();

    rerender(<Probe ready />);
    await flush();
    expect(useUiStore.getState().dialog).toBe("clone");
    expect(useCloneStore.getState().linkUrl).toBe("https://git.example.com/acme/plc.git");
  });

  it("opens the dialog when a link arrives, and lets it wait while not ready", async () => {
    const { rerender } = render(<Probe ready />);
    await flush();
    expect(useUiStore.getState().dialog).toBeNull();

    link.pending = "ssh://git@git.example.com:2222/acme/plc.git";
    rerender(<Probe ready={false} />);
    await act(async () => link.fire?.());
    expect(useUiStore.getState().dialog).toBeNull();

    rerender(<Probe ready />);
    await flush();
    expect(useUiStore.getState().dialog).toBe("clone");
    expect(useCloneStore.getState().linkUrl).toBe("ssh://git@git.example.com:2222/acme/plc.git");

    link.pending = "https://git.example.com/acme/hmi.git";
    await act(async () => link.fire?.());
    expect(useCloneStore.getState().linkUrl).toBe("https://git.example.com/acme/hmi.git");
  });
});
