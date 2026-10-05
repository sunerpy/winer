import type { HTMLAttributes, ReactNode } from "react";

import { cx } from "../lib/cx";

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  padding?: "none" | "sm" | "md";
  interactive?: boolean;
}

/** Surface, hairline, radius 10, no shadow: the one container in the app. */
export function Card({
  padding = "md",
  interactive = false,
  className,
  children,
  ...rest
}: CardProps) {
  return (
    <div
      className={cx(
        "min-w-0 rounded-10 bg-surface hairline",
        padding === "md" && "p-[var(--card-pad)]",
        padding === "sm" && "p-3",
        interactive && "transition-colors duration-150 hover:border-border-strong",
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}

export interface PanelProps extends Omit<CardProps, "title"> {
  /** The section label, in the locale's own words. */
  eyebrow: ReactNode;
  title?: ReactNode;
  /** Readouts, lamps or actions on the right of the header. */
  right?: ReactNode;
  bodyClassName?: string;
}

/** A card with an eyebrow header: `眉题 · 标题 ········· right`. */
export function Panel({
  eyebrow,
  title,
  right,
  children,
  className,
  bodyClassName,
  ...rest
}: PanelProps) {
  return (
    <Card className={cx("flex flex-col", className)} {...rest}>
      <header className="mb-3 flex min-h-5 flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex min-w-0 items-baseline gap-2">
          <h2 className="eyebrow whitespace-nowrap">{eyebrow}</h2>
          {title !== undefined && (
            <>
              <span className="eyebrow" aria-hidden>
                ·
              </span>
              <span className="truncate text-[12px] text-fg-muted">{title}</span>
            </>
          )}
        </div>
        {right !== undefined && (
          <div className="ml-auto flex min-w-0 items-center justify-end gap-2 text-[12px]">
            {right}
          </div>
        )}
      </header>
      <div className={cx("min-h-0 flex-1", bodyClassName)}>{children}</div>
    </Card>
  );
}

/** A labelled setting: label and help on the left, the control on the right, hairline below. */
export function Row({
  label,
  help,
  children,
  htmlFor,
  className,
}: {
  label: ReactNode;
  help?: ReactNode;
  children?: ReactNode;
  htmlFor?: string;
  className?: string;
}) {
  return (
    <div
      className={cx(
        "flex min-h-[52px] items-center justify-between gap-6 border-b border-border py-3 last:border-b-0",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <label htmlFor={htmlFor} className="block text-[14px] font-medium text-fg">
          {label}
        </label>
        {help !== undefined && <p className="mt-0.5 text-[12px] leading-4 text-fg-muted">{help}</p>}
      </div>
      <div className="flex max-w-[60%] min-w-0 shrink-0 items-center justify-end gap-2">
        {children}
      </div>
    </div>
  );
}
