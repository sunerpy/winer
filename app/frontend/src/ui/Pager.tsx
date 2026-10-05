// Page numbers for a list whose length is not known until its last page has arrived.
import { ChevronLeft, ChevronRight } from "lucide-react";

import { cx } from "../lib/cx";
import { IconButton } from "./Button";

export type PageItem = number | "gap";

/** The buttons to draw: the first and the last page known, the two either side of `current`, a gap
 *  for every run left out, and one more gap at the end while `more` pages may follow. */
export function pageItems(current: number, known: number, more: boolean): PageItem[] {
  const last = Math.max(1, known, current);
  const items: PageItem[] = [];
  for (let page = 1; page <= last; page += 1) {
    if (page === 1 || page === last || Math.abs(page - current) <= 2) items.push(page);
    else if (items.at(-1) !== "gap") items.push("gap");
  }
  if (more) items.push("gap");
  return items;
}

export interface PagerProps {
  page: number;
  /** Pages with something on them so far. */
  known: number;
  /** More pages may follow the known ones. */
  more: boolean;
  onPage: (page: number) => void;
  labels: { nav: string; previous: string; next: string; page: (page: number) => string };
}

export function Pager({ page, known, more, onPage, labels }: PagerProps) {
  return (
    <nav aria-label={labels.nav} className="flex items-center gap-0.5">
      <IconButton
        icon={ChevronLeft}
        label={labels.previous}
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
      />
      {pageItems(page, known, more).map((item, index) =>
        item === "gap" ? (
          <span
            // A gap has no identity of its own; its place in the row is it.
            key={`gap-${index}`}
            aria-hidden
            className="mono w-5 text-center text-[12px] text-fg-subtle"
          >
            …
          </span>
        ) : (
          <button
            key={item}
            type="button"
            aria-label={labels.page(item)}
            aria-current={item === page ? "page" : undefined}
            onClick={() => onPage(item)}
            className={cx(
              "mono h-7 min-w-7 rounded-6 px-1.5 text-[12px] transition-colors duration-150",
              item === page
                ? "bg-accent-soft font-semibold text-accent-text"
                : "text-fg-muted hover:text-fg hover-wash press-wash",
            )}
          >
            {item}
          </button>
        ),
      )}
      <IconButton
        icon={ChevronRight}
        label={labels.next}
        disabled={page >= known && !more}
        onClick={() => onPage(page + 1)}
      />
    </nav>
  );
}
