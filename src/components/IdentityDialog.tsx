import type { Identity } from "../bindings";
import { useIdentityForm } from "../hooks/useIdentityForm";
import { IdentityFields } from "./IdentityFields";
import { InlineError } from "./InlineError";
import { Modal } from "./Modal";

type Props = {
  open: boolean;
  initial: Identity;
  onClose: () => void;
  onSaved: () => void;
};

/** Asks for name and email and saves them to the global git config after explicit confirmation. */
export function IdentityDialog({ open, initial, onClose, onSaved }: Props) {
  const form = useIdentityForm(() => Promise.resolve(initial));

  return (
    <Modal open={open} title="Who's making these commits?" onClose={onClose}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (form.valid) void form.save().then((ok) => ok && onSaved());
        }}
      >
        <p>Git records a name and email on every commit you make.</p>
        <IdentityFields form={form} autoFocus />
        <p className="muted">
          This saves your name and email to your global Git settings, so every repository on this
          computer will use them.
        </p>
        <InlineError error={form.error} />
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" disabled={!form.valid || form.saving}>
            Save
          </button>
        </div>
      </form>
    </Modal>
  );
}
