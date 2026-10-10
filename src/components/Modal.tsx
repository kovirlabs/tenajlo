import { useEffect, useRef, type ReactNode } from "react";

type Props = {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  /** Extra class on the <dialog>, for dialogs with their own layout (Settings). */
  className?: string;
};

/** Native <dialog> modal: focus trap, Escape to close, backdrop. */
export function Modal({ open, title, onClose, children, className }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      className={className ? `dialog ${className}` : "dialog"}
      onClose={onClose}
      aria-label={title}
    >
      {open && (
        <>
          <h2>{title}</h2>
          {children}
        </>
      )}
    </dialog>
  );
}
