import type { ReactNode } from "react";

import { cx } from "../lib/cx";

export type Tone = "neutral" | "accent" | "ok" | "danger" | "warning" | "info" | "win" | "loss";

const BADGE: Record<Tone, string> = {
  neutral: "bg-inset text-fg-muted hairline",
  accent: "bg-accent-soft text-accent-text",
  ok: "bg-ok-soft text-ok-text",
  danger: "bg-danger-soft text-danger",
  warning: "bg-warning-soft text-warning",
  info: "bg-info-soft text-info",
  win: "bg-win-soft text-win",
  loss: "bg-loss-soft text-loss",
};

export function Badge({
  tone = "neutral",
  children,
  className,
  title,
}: {
  tone?: Tone;
  children: ReactNode;
  className?: string;
  title?: string;
}) {
  return (
    <span
      data-tone={tone}
      title={title}
      className={cx(
        "inline-flex h-5 shrink-0 items-center gap-1 rounded-4 px-1.5 text-[11px] leading-none font-medium whitespace-nowrap",
        BADGE[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

export type LampTone = "ok" | "warn" | "danger" | "accent" | "idle" | "off";

const LAMP: Record<LampTone, string> = {
  ok: "bg-ok",
  warn: "bg-warning",
  danger: "bg-danger",
  accent: "bg-accent",
  idle: "border-[1.5px] border-fg-subtle bg-transparent",
  off: "bg-border-strong",
};

/** A status dot. With a `label` it is an image with that name; otherwise decoration. */
export function Lamp({
  tone = "ok",
  size = 8,
  pulse = false,
  label,
  className,
}: {
  tone?: LampTone;
  size?: 6 | 8 | 10;
  pulse?: boolean;
  label?: string;
  className?: string;
}) {
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-tone={tone}
      className={cx("inline-block shrink-0 rounded-full", LAMP[tone], className)}
      style={{
        width: size,
        height: size,
        animation: pulse ? "winer-pulse 1.6s ease-in-out infinite" : undefined,
      }}
    />
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="mono inline-flex h-[18px] items-center rounded-4 bg-inset px-1 text-[10.5px] text-fg-muted hairline">
      {children}
    </kbd>
  );
}
