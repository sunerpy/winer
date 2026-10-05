import type { KeyboardEvent } from "react";

import { cx } from "../lib/cx";

export interface SegmentedOption<V extends string> {
  value: V;
  label: string;
  /** A tooltip, for an option whose label alone does not say enough. */
  title?: string;
  disabled?: boolean;
}

export interface SegmentedProps<V extends string> {
  options: readonly SegmentedOption<V>[];
  /** `null` while none of the options is the current state. */
  value: V | null;
  onChange: (value: V) => void;
  label: string;
  size?: "sm" | "md";
}

/** Arrow keys move a radio group's choice and its focus, wrapping, as in any radio group. Put it on
 *  the `radiogroup`; each radio carries `data-value`, and only the checked one sits in the Tab order. */
export function onRadioKeys<V extends string>(
  event: KeyboardEvent<HTMLElement>,
  values: readonly V[],
  value: V | null,
  onChange: (value: V) => void,
) {
  const step =
    event.key === "ArrowRight" || event.key === "ArrowDown"
      ? 1
      : event.key === "ArrowLeft" || event.key === "ArrowUp"
        ? -1
        : 0;
  if (!step || values.length === 0) return;
  event.preventDefault();
  const at = value === null ? -1 : values.indexOf(value);
  const next = values[(at + step + values.length) % values.length];
  if (next === undefined) return;
  onChange(next);
  event.currentTarget.querySelector<HTMLElement>(`[data-value="${next}"]`)?.focus();
}

/** A radio group drawn as one track. */
export function Segmented<V extends string>({
  options,
  value,
  onChange,
  label,
  size = "md",
}: SegmentedProps<V>) {
  const enabled = options.filter((option) => !option.disabled).map((option) => option.value);
  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={(event) => onRadioKeys(event, enabled, value, onChange)}
      className={cx(
        "inline-flex shrink-0 items-center rounded-6 bg-inset p-0.5 hairline",
        size === "sm" ? "h-7" : "h-8",
      )}
    >
      {options.map((option) => {
        const selected = option.value === value;
        // With nothing chosen the first enabled option takes the group's one Tab stop.
        const tabStop = selected || (value === null && option.value === enabled[0]);
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            data-value={option.value}
            title={option.title}
            aria-checked={selected}
            tabIndex={tabStop ? 0 : -1}
            disabled={option.disabled}
            onClick={() => onChange(option.value)}
            className={cx(
              "h-full rounded-[5px] px-3 whitespace-nowrap transition-colors duration-150 disabled:opacity-40",
              size === "sm" ? "text-[11.5px]" : "text-[12.5px]",
              selected
                ? "bg-surface font-medium text-fg shadow-[0_0_0_1px_var(--border)]"
                : "text-fg-muted hover:text-fg",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
