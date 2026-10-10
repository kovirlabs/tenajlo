import { useEffect, type DependencyList } from "react";

/**
 * `useEffect` for async work. `live()` turns false once `deps` change or the component
 * unmounts, so a late result can be dropped instead of overwriting newer state:
 *
 *   useAsyncEffect(async (live) => {
 *     const res = await getAppInfo();
 *     if (live()) setInfo(res);
 *   }, []);
 */
export function useAsyncEffect(
  effect: (live: () => boolean) => Promise<void>,
  deps: DependencyList,
) {
  useEffect(() => {
    let live = true;
    void effect(() => live);
    return () => {
      live = false;
    };
    // `deps` is the caller's list. The lint rule can't check it here, so list everything
    // `effect` reads, as for useEffect.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
