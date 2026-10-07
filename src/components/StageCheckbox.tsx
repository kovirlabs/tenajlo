import { useEffect, useRef } from "react";
import type { StagedState } from "../bindings";

type Props = {
  state: StagedState;
  label: string;
  disabled?: boolean;
  onChange: (stage: boolean) => void;
};

/** Checked = staged, indeterminate = partly staged. Clicking a partial file stages all of it. */
export function StageCheckbox({ state, label, disabled, onChange }: Props) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = state === "Partial";
  }, [state]);
  return (
    <input
      ref={ref}
      type="checkbox"
      checked={state === "Full"}
      disabled={disabled}
      aria-label={label}
      onClick={(e) => e.stopPropagation()}
      onChange={() => onChange(state !== "Full")}
    />
  );
}
