import {
  type ReactNode,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

import { cx } from "../lib/cx";

/** Closes when a pointer goes down outside `refs` or Escape is pressed. */
export function useDismiss(
  open: boolean,
  onClose: () => void,
  refs: RefObject<HTMLElement | null>[],
): void {
  useEffect(() => {
    if (!open) return undefined;
    const onPointer = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        refs.some((ref) => ref.current?.contains(event.target as Node))
      )
        return;
      onClose();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", onPointer, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, onClose, refs]);
}

export type Placement = "bottom-start" | "bottom-end" | "top-start" | "right-end";

/** Between the anchor and the panel, and between the panel and the window's edge. */
const GAP = 6;
const MARGIN = 8;

interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Where a `width` × `height` panel goes for `anchor` inside a `view`-sized window, all in window
 *  pixels: the preferred side first, the other side when the preferred one has no room, and never
 *  past the window's edges. */
export function placePanel(
  anchor: Box,
  width: number,
  height: number,
  placement: Placement,
  view: { width: number; height: number },
): { left: number; top: number } {
  const below = anchor.bottom + GAP;
  const above = anchor.top - GAP - height;
  let left: number;
  let top: number;
  switch (placement) {
    case "bottom-start":
      [left, top] = [anchor.left, below];
      break;
    case "bottom-end":
      [left, top] = [anchor.right - width, below];
      break;
    case "top-start":
      [left, top] = [anchor.left, above];
      break;
    case "right-end":
      [left, top] = [anchor.right + GAP + 2, anchor.bottom - height];
      break;
  }
  const fitsBelow = below + height <= view.height - MARGIN;
  const fitsAbove = above >= MARGIN;
  if (placement.startsWith("bottom") && !fitsBelow && fitsAbove) top = above;
  if (placement === "top-start" && !fitsAbove && fitsBelow) top = below;
  const clamp = (value: number, max: number) => Math.max(MARGIN, Math.min(value, max - MARGIN));
  return { left: clamp(left, view.width - width), top: clamp(top, view.height - height) };
}

export interface PopoverProps {
  open: boolean;
  onClose: () => void;
  anchor: RefObject<HTMLElement | null>;
  /** The preferred side; the panel moves to the other one when the window has no room. */
  placement?: Placement;
  children: ReactNode;
  className?: string;
  label?: string;
}

/**
 * A floating panel next to its anchor, drawn in a layer of its own (a portal, fixed position).
 * Drawn inside the page instead, a panel near an edge widened the scrolling content, and its
 * autofocused field then scrolled the whole page sideways to reach it.
 */
export function Popover({
  open,
  onClose,
  anchor,
  placement = "bottom-start",
  children,
  className,
  label,
}: PopoverProps) {
  const panel = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  useDismiss(open, onClose, [panel, anchor]);

  useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return undefined;
    }
    const place = () => {
      const element = panel.current;
      const host = anchor.current?.getBoundingClientRect();
      if (!element || !host) return;
      const box = element.getBoundingClientRect();
      // Rectangles are in window pixels and `left`/`top` in CSS pixels; the 字号 setting zooms
      // the page, which makes the two differ by exactly the zoom.
      const scale = element.offsetWidth > 0 ? box.width / element.offsetWidth : 1;
      const at = placePanel(host, box.width, box.height, placement, {
        width: window.innerWidth,
        height: window.innerHeight,
      });
      setPosition({ left: at.left / scale, top: at.top / scale });
    };
    place();
    window.addEventListener("resize", place);
    // Any scroller moving the anchor, in capture so nested scrollers count too.
    window.addEventListener("scroll", place, true);
    const resized = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
    if (panel.current) resized?.observe(panel.current);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      resized?.disconnect();
    };
  }, [open, placement, anchor]);

  if (!open) return null;
  return createPortal(
    <div
      ref={panel}
      role="dialog"
      aria-label={label}
      className={cx("fixed z-[60] rounded-10 bg-raised p-1 shadow-pop hairline", className)}
      // The first commit measures at the origin; the layout effect places it before anything is
      // painted. Not hidden meanwhile: a hidden field cannot take the autofocus it asks for.
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        animation: "winer-rise 150ms ease-out",
      }}
    >
      {children}
    </div>,
    document.body,
  );
}
