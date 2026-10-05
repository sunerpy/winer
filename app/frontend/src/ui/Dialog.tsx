import { X } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";

import { cx } from "../lib/cx";
import { IconButton } from "./Button";

const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

/** Focusables that can really take focus. `closest('[hidden]')` rather than `offsetParent`, which is
 *  null inside any fixed layer (and always in jsdom), and would switch the trap off silently. */
function focusables(panel: HTMLElement): HTMLElement[] {
  return [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) =>
      !element.hasAttribute("disabled") &&
      !element.closest("[hidden]") &&
      element.getAttribute("aria-hidden") !== "true",
  );
}

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  /** Required: the corner ✕ is the primary exit and needs a name. */
  closeLabel: string;
  children: ReactNode;
  footer?: ReactNode;
  /** A fixed frame (settings) instead of one sized by its content. */
  size?: "sm" | "lg";
  /** Off by default: each call site decides whether a stray click may discard what is open. */
  dismissOnScrim?: boolean;
  className?: string;
}

export function Dialog({
  open,
  onClose,
  title,
  closeLabel,
  children,
  footer,
  size = "sm",
  dismissOnScrim = false,
  className,
}: DialogProps) {
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();

  // Focus moves in on open and goes back to whatever had it on close, if that still exists.
  useEffect(() => {
    if (!open) return undefined;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const current = panel.current;
    const target =
      current?.querySelector<HTMLElement>("[data-autofocus]") ??
      (current ? focusables(current)[0] : undefined);
    target?.focus();
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  }, [open]);

  if (!open) return null;

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab" || !panel.current) return;
    const items = focusables(panel.current);
    const [first, last] = [items[0], items.at(-1)];
    if (!first || !last) return;
    const active = document.activeElement;
    // Only the edges are intercepted: the browser's own order is right everywhere in between.
    if (!panel.current.contains(active)) {
      event.preventDefault();
      first.focus();
    } else if (event.shiftKey && active === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center" onKeyDown={onKeyDown}>
      <div
        aria-hidden
        data-testid="dialog-scrim"
        className="absolute inset-0 bg-scrim"
        style={{ animation: "winer-fade 150ms ease-out" }}
        onClick={dismissOnScrim ? onClose : undefined}
      />
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={cx(
          "relative flex flex-col overflow-hidden rounded-14 bg-raised shadow-win hairline",
          size === "lg"
            ? "h-[min(600px,calc(100vh-96px))] w-[min(820px,calc(100vw-64px))]"
            : "max-h-[calc(100vh-96px)] w-[min(460px,calc(100vw-64px))]",
          className,
        )}
        style={{ animation: "winer-rise 200ms ease-out" }}
      >
        <header className="flex h-12 shrink-0 items-center justify-between gap-3 border-b border-border pr-2 pl-5">
          <h2 id={titleId} className="truncate text-[14px] font-semibold text-fg">
            {title}
          </h2>
          <IconButton icon={X} label={closeLabel} onClick={onClose} />
        </header>
        <div className="flex min-h-0 flex-1 flex-col">{children}</div>
        {footer !== undefined && (
          <footer className="flex shrink-0 items-center justify-end gap-2 border-t border-border px-5 py-3">
            {footer}
          </footer>
        )}
      </div>
    </div>,
    document.body,
  );
}
