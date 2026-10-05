// Appearance lives in the core's settings; <html> attributes make every token follow it.
import type { Appearance, Theme } from "@winer/shared";
import { useEffect, useState } from "react";

export type ResolvedTheme = Exclude<Theme, "system">;

const DARK_THEMES: ReadonlySet<ResolvedTheme> = new Set(["dark", "graphite", "hextech"]);
const HINT_KEY = "winer.appearance";

export function resolveTheme(theme: Theme, systemDark: boolean): ResolvedTheme {
  if (theme === "system") return systemDark ? "dark" : "light";
  return theme;
}

export function modeOf(theme: ResolvedTheme): "light" | "dark" {
  return DARK_THEMES.has(theme) ? "dark" : "light";
}

export function applyAppearance(
  appearance: Appearance,
  systemDark: boolean,
  root = document.documentElement,
): void {
  const theme = resolveTheme(appearance.theme, systemDark);
  root.dataset.theme = theme;
  root.dataset.mode = modeOf(theme);
  root.dataset.accent = appearance.accent;
  root.dataset.density = appearance.density;
  root.dataset.reduceMotion = String(appearance.reduceMotion);
  // 13px is the size the design is drawn at; other sizes zoom the whole window.
  root.style.setProperty("--ui-zoom", String(appearance.fontSize / 13));
  try {
    localStorage.setItem(HINT_KEY, JSON.stringify(appearance));
  } catch {
    // Storage can be disabled in a webview; the hint is only a first-paint nicety.
  }
}

/** The last appearance, so the first frame is not the default theme while settings load. The
 *  settings the core returns always overwrite it. */
export function applyAppearanceHint(): void {
  try {
    const hint = localStorage.getItem(HINT_KEY);
    if (hint) applyAppearance(JSON.parse(hint) as Appearance, systemPrefersDark());
  } catch {
    // A missing or malformed hint changes nothing.
  }
}

export function systemPrefersDark(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches;
}

export function useSystemDark(): boolean {
  const [dark, setDark] = useState(systemPrefersDark);
  useEffect(() => {
    if (typeof matchMedia !== "function") return;
    const query = matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return dark;
}
