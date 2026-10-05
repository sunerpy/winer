import type { LucideIcon } from "lucide-react";
import { type InputHTMLAttributes, forwardRef, useEffect, useRef, useState } from "react";

import { cx } from "../lib/cx";

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  icon?: LucideIcon;
  invalid?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { icon: Icon, invalid = false, className, ...rest },
  ref,
) {
  return (
    <span className={cx("relative inline-flex h-8 min-w-0 items-center", className)}>
      {Icon && (
        <Icon
          size={14}
          strokeWidth={2}
          aria-hidden
          className="pointer-events-none absolute left-2.5 text-fg-subtle"
        />
      )}
      <input
        ref={ref}
        aria-invalid={invalid || undefined}
        className={cx(
          "h-full w-full min-w-0 rounded-6 bg-surface text-[13px] text-fg outline-none hairline transition-colors duration-150 placeholder:text-fg-subtle",
          "hover:border-border-strong focus:border-accent focus-visible:outline-none disabled:opacity-50",
          invalid && "border-danger!",
          Icon ? "pr-2.5 pl-8" : "px-2.5",
        )}
        {...rest}
      />
    </span>
  );
});

export interface CommitInputProps extends Omit<InputProps, "value" | "defaultValue" | "onChange"> {
  value: string;
  onCommit: (value: string) => void;
}

/** A text setting committed when the field is left or Enter is pressed, not on every key, and
 *  only when the text changed while the field had focus. */
export function CommitInput({
  value,
  onCommit,
  onFocus,
  onBlur,
  onKeyDown,
  ...rest
}: CommitInputProps) {
  const [draft, setDraft] = useState(value);
  const editing = useRef(false);
  const before = useRef(value);
  // A saved value comes back normalized (trimmed, clipped); show it unless the user is typing.
  useEffect(() => {
    if (!editing.current) setDraft(value);
  }, [value]);
  return (
    <Input
      {...rest}
      value={draft}
      onFocus={(event) => {
        editing.current = true;
        before.current = draft;
        onFocus?.(event);
      }}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={(event) => {
        editing.current = false;
        if (draft !== before.current) onCommit(draft);
        onBlur?.(event);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        onKeyDown?.(event);
      }}
    />
  );
}

export interface SliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  /** Called once the hand settles, not on every step of a drag. */
  onChange: (value: number) => void;
  label: string;
  /** Shown to the right, e.g. `1.5 秒`; follows the thumb while it moves. */
  format?: (value: number) => string;
}

/** A native range input, styled: keyboard, pointer and accessibility come with it. */
export function Slider({ value, min, max, step = 1, onChange, label, format }: SliderProps) {
  const [draft, setDraft] = useState(value);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => setDraft(value), [value]);
  useEffect(() => () => clearTimeout(timer.current), []);

  const move = (next: number) => {
    setDraft(next);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => onChange(next), 300);
  };
  const fill = ((draft - min) / (max - min)) * 100;
  return (
    <span className="inline-flex items-center gap-3">
      <input
        type="range"
        aria-label={label}
        aria-valuetext={format?.(draft)}
        value={draft}
        min={min}
        max={max}
        step={step}
        onChange={(event) => move(Number(event.target.value))}
        className="h-1 w-40 cursor-pointer appearance-none rounded-pill bg-transparent [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-thumb [&::-webkit-slider-thumb]:shadow-thumb [&::-webkit-slider-thumb]:ring-2 [&::-webkit-slider-thumb]:ring-accent"
        style={{
          background: `linear-gradient(to right, var(--accent) ${fill}%, var(--inset2) ${fill}%)`,
        }}
      />
      {format && (
        <span className="mono w-14 text-right text-[12px] text-fg-muted">{format(draft)}</span>
      )}
    </span>
  );
}
