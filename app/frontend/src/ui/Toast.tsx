import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { useEffect } from "react";

import { cx } from "../lib/cx";
import { Observable, useObservable } from "../lib/observable";

export type ToastTone = "ok" | "danger" | "info";

interface ToastEntry {
  id: number;
  message: string;
  tone: ToastTone;
}

const toasts = new Observable<readonly ToastEntry[]>([]);
let nextId = 0;

/** Shows a short message bottom-right; it leaves on its own after a few seconds. */
export function toast(message: string, tone: ToastTone = "info"): void {
  const entry = { id: ++nextId, message, tone };
  toasts.update((list) => [...list, entry].slice(-4));
}

function dismiss(id: number): void {
  toasts.update((list) => list.filter((entry) => entry.id !== id));
}

const ICON = { ok: CircleCheck, danger: CircleAlert, info: Info } as const;
const TONE = { ok: "text-ok", danger: "text-danger", info: "text-accent-text" } as const;

function ToastItem({ entry, closeLabel }: { entry: ToastEntry; closeLabel: string }) {
  useEffect(() => {
    const timer = setTimeout(() => dismiss(entry.id), entry.tone === "danger" ? 7000 : 4000);
    return () => clearTimeout(timer);
  }, [entry]);
  const Icon = ICON[entry.tone];
  return (
    <li
      role={entry.tone === "danger" ? "alert" : "status"}
      className="pointer-events-auto flex w-[340px] items-start gap-2.5 rounded-10 bg-raised py-2.5 pr-1.5 pl-3 shadow-pop hairline"
      style={{ animation: "winer-rise 200ms ease-out" }}
    >
      <Icon
        size={16}
        strokeWidth={2}
        className={cx("mt-0.5 shrink-0", TONE[entry.tone])}
        aria-hidden
      />
      <p className="min-w-0 flex-1 text-[12.5px] leading-5 break-words text-fg">{entry.message}</p>
      <button
        type="button"
        aria-label={closeLabel}
        onClick={() => dismiss(entry.id)}
        className="grid size-6 shrink-0 place-items-center rounded-6 text-fg-subtle hover:text-fg hover-wash"
      >
        <X size={13} strokeWidth={2} />
      </button>
    </li>
  );
}

export function Toaster({ closeLabel }: { closeLabel: string }) {
  const list = useObservable(toasts);
  return (
    <ol
      aria-live="polite"
      className="pointer-events-none fixed right-5 bottom-10 z-60 flex flex-col items-end gap-2"
    >
      {list.map((entry) => (
        <ToastItem key={entry.id} entry={entry} closeLabel={closeLabel} />
      ))}
    </ol>
  );
}
