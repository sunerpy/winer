// The self-drawn title bar's window handle and maximize state.
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useState } from "react";

import { currentPlatform, type Platform } from "./platform";

// Function properties, not methods: every member is a closure, safe to pass around unbound.
export interface WindowControls {
  minimize: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  close: () => Promise<void>;
  isMaximized: () => Promise<boolean>;
  startDragging: () => Promise<void>;
  /** The resize payload is dropped on purpose: maximize state is always re-queried. */
  onResized: (handler: () => void) => Promise<UnlistenFn>;
}

/** `null` outside a Tauri webview, where `getCurrentWindow()` throws rather than returning a
 *  dead handle; the bar then draws no buttons instead of three that do nothing. */
export function createWindowControls(): WindowControls | null {
  try {
    const window = getCurrentWindow();
    return {
      minimize: () => window.minimize(),
      toggleMaximize: () => window.toggleMaximize(),
      // `close()` raises close-requested, so the shell's tray policy still decides.
      close: () => window.close(),
      isMaximized: () => window.isMaximized(),
      startDragging: () => window.startDragging(),
      onResized: (handler) => window.onResized(() => handler()),
    };
  } catch {
    return null;
  }
}

function unsubscribeQuietly(unlisten: UnlistenFn): void {
  try {
    void Promise.resolve(unlisten()).catch(() => undefined);
  } catch {
    // An exception out of an effect cleanup would unmount the whole tree.
  }
}

export interface WindowChrome {
  platform: Platform;
  controls: WindowControls | null;
  maximized: boolean;
  minimize: () => void;
  toggleMaximize: () => void;
  close: () => void;
  /** Tauri's drag script listens to the mouse only; touch and pen start the drag here. */
  dragOnNonMousePointer: (pointerType: string) => void;
}

export function useWindowChrome(
  factory: () => WindowControls | null = createWindowControls,
  platformOf: () => Platform = currentPlatform,
): WindowChrome {
  const controls = useMemo(() => factory(), [factory]);
  const platform = useMemo(() => platformOf(), [platformOf]);
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!controls) return undefined;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    const sync = async () => {
      try {
        const value = await controls.isMaximized();
        if (!disposed) setMaximized(value);
      } catch {
        // Keep the last known state on screen.
      }
    };
    // A window restored maximized emits no resize at start-up, so the first state is asked for.
    void sync();
    controls
      .onResized(() => void sync())
      .then((fn) => {
        if (disposed) unsubscribeQuietly(fn);
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      if (unlisten) unsubscribeQuietly(unlisten);
    };
  }, [controls]);

  const run = useCallback((action: (() => Promise<void>) | undefined) => {
    if (action) void action().catch(() => undefined);
  }, []);

  return {
    platform,
    controls,
    maximized,
    minimize: useCallback(() => run(controls?.minimize), [controls, run]),
    toggleMaximize: useCallback(() => run(controls?.toggleMaximize), [controls, run]),
    close: useCallback(() => run(controls?.close), [controls, run]),
    dragOnNonMousePointer: useCallback(
      (pointerType: string) => {
        if (pointerType !== "mouse") run(controls?.startDragging);
      },
      [controls, run],
    ),
  };
}
