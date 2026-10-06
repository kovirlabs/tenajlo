import { useEffect, useRef } from "react";
import type { StagedState } from "../bindings";

type Props = { state: StagedState; label: string };

/** Shows how much of a file is staged. Read-only until staging lands in M2. */
export function StageCheckbox({ state, label }: Props) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = state === "Partial";
  }, [state]);
  return (
    <input
      ref={ref}
      type="checkbox"
      checked={state === "Full"}
      disabled
      readOnly
      aria-label={label}
    />
  );
}
