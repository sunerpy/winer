import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cx } from "../lib/cx";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "accent" | "outline" | "ghost" | "danger" | "link";

const VARIANT: Record<ButtonVariant, string> = {
  primary: "bg-primary text-primary-fg hover:opacity-90 active:opacity-80",
  accent: "bg-accent text-accent-fg hover:brightness-105 active:brightness-95",
  outline: "bg-surface text-fg hairline hover:border-border-strong hover-wash press-wash",
  ghost: "text-fg hover-wash press-wash",
  danger: "bg-danger text-white hover:opacity-90 active:opacity-80",
  link: "px-0! text-accent-text hover:underline underline-offset-4",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  icon?: LucideIcon;
  /** Busy: the label stays (so the width does not jump) and a spinner replaces the glyph. */
  loading?: boolean;
  children?: ReactNode;
}

export function Button({
  variant = "outline",
  size = "md",
  icon: Icon,
  loading = false,
  disabled,
  className,
  children,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      data-variant={variant}
      className={cx(
        "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-6 font-medium whitespace-nowrap transition-[background-color,border-color,opacity,filter] duration-150",
        "disabled:opacity-50",
        size === "sm" ? "h-7 px-2.5 text-[12px]" : "h-8 px-3 text-[13px]",
        VARIANT[variant],
        className,
      )}
      {...rest}
    >
      {loading ? (
        <Spinner size={size === "sm" ? 12 : 14} />
      ) : (
        Icon && <Icon size={size === "sm" ? 13 : 14} strokeWidth={2} />
      )}
      {children}
    </button>
  );
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: LucideIcon;
  /** Required: a glyph alone is not a name. Also the tooltip. */
  label: string;
  size?: 24 | 28 | 32;
  tone?: "default" | "danger";
}

export function IconButton({
  icon: Icon,
  label,
  size = 28,
  tone = "default",
  className,
  type = "button",
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={cx(
        "inline-flex shrink-0 items-center justify-center rounded-6 text-fg-muted transition-colors duration-150 hover:text-fg hover-wash press-wash disabled:opacity-40",
        tone === "danger" && "hover:text-danger",
        className,
      )}
      style={{ width: size, height: size }}
      {...rest}
    >
      <Icon size={size === 24 ? 14 : 16} strokeWidth={2} />
    </button>
  );
}
