import { useEffect, useEffectEvent } from "react";

/** Subscribes with an `api/` listener (`onProgress`, `onAuthPrompt`…); resolves to its unlisten. */
type Listen<T> = (callback: (payload: T) => void) => Promise<() => void>;

/**
 * Calls `handler` for each Tauri event while the component is mounted. `handler` always sees
 * the latest props and state. Listening is async, so an unlisten that arrives after unmount
 * runs right away.
 */
export function useTauriEvent<T>(listen: Listen<T>, handler: (payload: T) => void) {
  const onEvent = useEffectEvent(handler);
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void listen((payload) => onEvent(payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [listen]);
}
