// The 40px strip that is the window's title bar: page title, a compact readout, the right slot and,
// on Windows and Linux, the window controls. `deep` makes the title and readout drag too; Tauri's
// script already skips buttons and toggles maximize on double click, so there is no handler here.
import { Copy, Minus, Square, X } from "lucide-react";
import type { ReactNode } from "react";

import { useT } from "../lib/i18n";
import { cx } from "../lib/cx";
import type { WindowChrome } from "../lib/window";

const CONTROL =
  "inline-flex h-full w-11 items-center justify-center text-fg-muted outline-none transition-colors duration-150 hover:bg-nav-active hover:text-fg focus-visible:bg-nav-active focus-visible:text-fg";

export function TitleBar({
  title,
  readout,
  right,
  chrome,
  insetForTrafficLights,
}: {
  title: ReactNode;
  readout?: ReactNode;
  right?: ReactNode;
  chrome: WindowChrome;
  /** The sidebar is hidden on macOS: clear the traffic lights. */
  insetForTrafficLights?: boolean;
}) {
  const t = useT();
  const showControls = chrome.platform !== "macos" && chrome.controls !== null;
  return (
    <header
      data-tauri-drag-region="deep"
      data-platform={chrome.platform}
      onPointerDown={(event) => chrome.dragOnNonMousePointer(event.pointerType)}
      className="flex h-10 shrink-0 items-center border-b border-border bg-surface select-none"
    >
      <div
        className={cx(
          "flex min-w-0 flex-1 items-center gap-3 pr-4",
          insetForTrafficLights ? "pl-20" : "pl-6",
        )}
      >
        <h1 className="shrink-0 truncate text-[14px] font-semibold text-fg">{title}</h1>
        {readout !== undefined && (
          <div className="mono hidden min-w-0 flex-1 items-center gap-1.5 truncate text-[11px] text-fg-subtle md:flex">
            {readout}
          </div>
        )}
        {readout === undefined && <span className="flex-1" />}
        {right !== undefined && <div className="flex shrink-0 items-center gap-1.5">{right}</div>}
      </div>
      {showControls && (
        <div className="flex h-full shrink-0 items-stretch">
          <button
            type="button"
            aria-label={t("window.minimize")}
            title={t("window.minimize")}
            onClick={chrome.minimize}
            className={CONTROL}
          >
            <Minus size={15} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            data-state={chrome.maximized ? "maximized" : "normal"}
            aria-label={chrome.maximized ? t("window.restore") : t("window.maximize")}
            title={chrome.maximized ? t("window.restore") : t("window.maximize")}
            onClick={chrome.toggleMaximize}
            className={CONTROL}
          >
            {chrome.maximized ? (
              <Copy size={13} strokeWidth={1.75} />
            ) : (
              <Square size={12} strokeWidth={1.75} />
            )}
          </button>
          <button
            type="button"
            aria-label={t("window.close")}
            title={t("window.close")}
            onClick={chrome.close}
            className={cx(
              CONTROL,
              "hover:bg-danger! hover:text-white! focus-visible:bg-danger! focus-visible:text-white!",
            )}
          >
            <X size={15} strokeWidth={1.75} />
          </button>
        </div>
      )}
    </header>
  );
}
