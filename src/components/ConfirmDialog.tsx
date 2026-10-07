import type { ReactNode } from "react";
import { Modal } from "./Modal";

type Props = {
  open: boolean;
  title: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  children: ReactNode;
};

export function ConfirmDialog({ open, title, confirmLabel, onConfirm, onCancel, children }: Props) {
  return (
    <Modal open={open} title={title} onClose={onCancel}>
      {children}
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onCancel} autoFocus>
          Cancel
        </button>
        <button type="button" className="danger" onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}
