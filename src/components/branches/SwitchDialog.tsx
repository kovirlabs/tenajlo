import { useState } from "react";
import type { LocalChanges } from "../../bindings";
import { Modal } from "../Modal";

type Props = {
  target: string | null;
  current: string;
  onCancel: () => void;
  onSwitch: (choice: LocalChanges) => void;
};

/** Asked when switching with uncommitted changes (GitHub Desktop's two options). */
export function SwitchDialog({ target, current, onCancel, onSwitch }: Props) {
  const [choice, setChoice] = useState<LocalChanges>("Leave");
  return (
    <Modal open={target !== null} title="You have uncommitted changes" onClose={onCancel}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          onSwitch(choice);
        }}
      >
        <label className="choice">
          <input
            type="radio"
            name="local-changes"
            checked={choice === "Leave"}
            onChange={() => setChoice("Leave")}
          />
          <span>
            Leave my changes on <strong>{current}</strong>
            <small className="muted">They'll be waiting for you when you come back.</small>
          </span>
        </label>
        <label className="choice">
          <input
            type="radio"
            name="local-changes"
            checked={choice === "Bring"}
            onChange={() => setChoice("Bring")}
          />
          <span>
            Bring my changes to <strong>{target}</strong>
            <small className="muted">They'll move with you to the other branch.</small>
          </span>
        </label>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit">Switch branch</button>
        </div>
      </form>
    </Modal>
  );
}
