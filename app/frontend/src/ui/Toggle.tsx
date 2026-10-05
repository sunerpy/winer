import { useId } from "react";

import { cx } from "../lib/cx";

export interface ToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  /** The accessible name when no visible label is next to it. */
  label?: string;
  id?: string;
  disabled?: boolean;
}

/** A 32×19 switch: the accent when on, a faint track when off, one thumb in both. */
export function Toggle({ checked, onChange, label, id, disabled = false }: ToggleProps) {
  const fallback = useId();
  return (
    <button
      id={id ?? fallback}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "relative inline-flex h-[19px] w-8 shrink-0 items-center rounded-pill transition-colors duration-150 disabled:opacity-50",
        checked ? "bg-accent hover:brightness-105" : "bg-fg/15 hover:bg-fg/20",
      )}
    >
      <span
        aria-hidden
        className={cx(
          "absolute top-[3px] size-[13px] rounded-full bg-thumb shadow-thumb transition-transform duration-150",
          checked ? "translate-x-4" : "translate-x-[3px]",
        )}
      />
    </button>
  );
}
