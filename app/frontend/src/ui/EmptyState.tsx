import type { LucideIcon } from "lucide-react";
import type { CSSProperties, ReactNode } from "react";

import { cx } from "../lib/cx";

/** Says what is missing and what to do about it. */
export function EmptyState({
  icon: Icon,
  title,
  children,
  actions,
  compact = false,
  className,
}: {
  icon?: LucideIcon;
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
  className?: string;
}) {
  return (
    <div
      role="status"
      className={cx(
        "flex flex-col items-center justify-center text-center",
        compact ? "gap-2 py-6" : "gap-3 py-14",
        className,
      )}
    >
      {Icon && (
        <span
          className={cx(
            "grid place-items-center rounded-10 bg-inset text-fg-subtle hairline",
            compact ? "size-9" : "size-11",
          )}
        >
          <Icon size={compact ? 16 : 20} strokeWidth={1.75} />
        </span>
      )}
      <p className={cx("font-medium text-fg", compact ? "text-[13px]" : "text-[15px]")}>{title}</p>
      {children !== undefined && (
        <div className="max-w-[460px] text-[12.5px] leading-5 text-fg-muted">{children}</div>
      )}
      {actions !== undefined && <div className="mt-1 flex items-center gap-2">{actions}</div>}
    </div>
  );
}

/** A failed load: what happened in one plain line, the client's own words below it, a retry. */
export function ErrorNote({
  title,
  detail,
  retryLabel,
  onRetry,
}: {
  title: string;
  detail?: string;
  retryLabel: string;
  onRetry?: () => void;
}) {
  return (
    <div role="status" className="flex flex-col items-center gap-2 py-6 text-center">
      <p className="text-[13px] font-medium text-fg">{title}</p>
      {detail && (
        <p className="mono max-w-[560px] text-[11px] leading-4 break-words text-fg-subtle">
          {detail}
        </p>
      )}
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="mt-1 h-7 rounded-6 px-2.5 text-[12px] font-medium text-accent-text hover-wash"
        >
          {retryLabel}
        </button>
      )}
    </div>
  );
}

/** A pulsing placeholder shaped like the content it stands in for. */
export function Skeleton({ className, style }: { className?: string; style?: CSSProperties }) {
  return (
    <span
      aria-hidden
      className={cx("block rounded-6 bg-inset2", className)}
      style={{ animation: "winer-shimmer 1.4s ease-in-out infinite", ...style }}
    />
  );
}
