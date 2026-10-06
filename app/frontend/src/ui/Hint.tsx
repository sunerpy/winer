import { CircleHelp } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { cx } from "../lib/cx";
import { type Placement, Popover } from "./Popover";

/** A help glyph beside a figure that opens the rule behind it. The glyph's name (and tooltip) is
 *  `label`; the panel takes the focus when it opens, so a screen reader reads it, and gives it
 *  back when Esc closes it. */
export function Hint({
  label,
  title,
  children,
  placement = "bottom-start",
  className,
}: {
  label: string;
  title?: ReactNode;
  children: ReactNode;
  placement?: Placement;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (open) body.current?.focus();
  }, [open]);
  const close = () => {
    const inside = body.current?.contains(document.activeElement) ?? false;
    setOpen(false);
    if (inside) button.current?.focus();
  };
  return (
    <span ref={anchor} className={cx("inline-flex shrink-0", className)}>
      <button
        ref={button}
        type="button"
        aria-label={label}
        title={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="inline-flex size-5 items-center justify-center rounded-4 text-fg-subtle transition-colors duration-150 hover:text-fg hover-wash"
      >
        <CircleHelp size={14} strokeWidth={2} aria-hidden />
      </button>
      <Popover
        open={open}
        onClose={close}
        anchor={anchor}
        placement={placement}
        label={label}
        className="w-[min(380px,calc(100vw-16px))] p-3"
      >
        <div ref={body} tabIndex={-1} className="outline-none">
          {title !== undefined && (
            <p className="mb-1.5 text-[12.5px] font-medium text-fg">{title}</p>
          )}
          <div className="flex flex-col gap-1.5 text-[12px] leading-[18px] text-fg-muted">
            {children}
          </div>
        </div>
      </Popover>
    </span>
  );
}
