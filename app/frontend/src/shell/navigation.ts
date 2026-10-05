import {
  Bot,
  History,
  LayoutDashboard,
  type LucideIcon,
  Puzzle,
  Swords,
  Wrench,
} from "lucide-react";
import { createContext, useContext } from "react";

import type { MessageKey } from "../lib/i18n";

export type Page = "overview" | "live" | "history" | "automation" | "tools" | "plugin";

/** Where the window is. History can be opened on someone else's games. */
export type Route = { page: Exclude<Page, "history"> } | { page: "history"; puuid?: string };

export type SettingsSection = "appearance" | "general" | "rating" | "about";

export interface NavItem {
  page: Page;
  label: MessageKey;
  icon: LucideIcon;
}

export const NAV_GROUPS: { title: MessageKey; items: NavItem[] }[] = [
  {
    title: "nav.workbench",
    items: [
      { page: "overview", label: "nav.overview", icon: LayoutDashboard },
      { page: "live", label: "nav.live", icon: Swords },
      { page: "history", label: "nav.history", icon: History },
    ],
  },
  {
    title: "nav.assistant",
    items: [
      { page: "automation", label: "nav.automation", icon: Bot },
      { page: "tools", label: "nav.tools", icon: Wrench },
      { page: "plugin", label: "nav.plugin", icon: Puzzle },
    ],
  },
];

/** Ctrl+1 … Ctrl+6 follow this order. */
export const PAGE_ORDER: Page[] = NAV_GROUPS.flatMap((group) =>
  group.items.map((item) => item.page),
);

export interface ShellApi {
  route: Route;
  navigate: (route: Route) => void;
  openSettings: (section?: SettingsSection) => void;
}

export const ShellContext = createContext<ShellApi | null>(null);

export function useShell(): ShellApi {
  const shell = useContext(ShellContext);
  if (!shell) throw new Error("useShell outside the shell");
  return shell;
}
