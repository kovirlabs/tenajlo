import { useEffect } from "react";
import { onCloneLink, takeCloneLink } from "../api/openLink";
import { useCloneStore } from "../stores/cloneStore";
import { useTauriEvent } from "./useTauriEvent";

async function takeAndShow() {
  const url = await takeCloneLink();
  if (url) useCloneStore.getState().showLink(url);
}

/**
 * Opens the Clone dialog for Open in Tenajlo links (spec §8.3) once `ready`: the main screen
 * is up and no other dialog or clone is in the way. Until then the link waits in Rust, so a
 * link that started Tenajlo, or arrived during the welcome screen, opens the dialog later.
 */
export function useCloneLinks(ready: boolean) {
  useEffect(() => {
    if (ready) void takeAndShow();
  }, [ready]);

  useTauriEvent(onCloneLink, () => {
    if (ready) void takeAndShow();
  });
}
