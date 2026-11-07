import { commands, events } from "../bindings";

/**
 * The clone URL from an Open in Tenajlo link (spec §8.3) that hasn't been shown yet, or null.
 * Rust has already checked it; taking it clears it.
 */
export function takeCloneLink(): Promise<string | null> {
  return commands.takeCloneLink();
}

/** Called when an Open in Tenajlo link arrives. Take it with `takeCloneLink` when ready. */
export function onCloneLink(cb: (url: string) => void): Promise<() => void> {
  return events.cloneLinkReceived.listen((e) => cb(e.payload.url));
}
