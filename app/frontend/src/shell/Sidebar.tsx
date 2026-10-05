// The full-height nav: a brand row that is part of the window's drag strip, grouped views, and
// footer entries that open things rather than navigate.
import type { Theme } from "@winer/shared";
import {
  Check,
  type LucideIcon,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Sun,
  SunMoon,
} from "lucide-react";
import { useRef, useState } from "react";

import { useT } from "../lib/i18n";
import { cx } from "../lib/cx";
import { useLive, useSettings, useStore } from "../lib/store";
import { Lamp, Popover } from "../ui";
import { Logo } from "./Logo";
import { NAV_GROUPS, type Page } from "./navigation";

export const SIDEBAR_WIDTH = 224;
export const RAIL_WIDTH = 56;
/** macOS keeps its traffic lights over the top-left corner (`trafficLightPosition` x 14). */
const TRAFFIC_LIGHTS_INSET = 80;

const ROW = "flex h-9 w-full items-center rounded-6 text-[13px] transition-colors duration-150";

function Entry({
  icon: Icon,
  label,
  collapsed,
  current = false,
  onClick,
  popup,
  expanded,
}: {
  icon: LucideIcon;
  label: string;
  collapsed: boolean;
  current?: boolean;
  onClick: () => void;
  /** Opens a dialog or a menu instead of navigating. */
  popup?: "dialog" | "menu";
  expanded?: boolean;
}) {
  return (
    <button
      type="button"
      aria-current={current ? "page" : undefined}
      aria-haspopup={popup}
      aria-expanded={popup === "menu" ? expanded : undefined}
      aria-label={collapsed ? label : undefined}
      title={collapsed ? label : undefined}
      onClick={onClick}
      className={cx(
        ROW,
        collapsed ? "justify-center px-0" : "gap-2.5 px-2.5",
        current ? "bg-nav-active font-medium text-fg" : "text-fg-muted hover:text-fg hover-wash",
      )}
    >
      <Icon
        size={16}
        strokeWidth={2}
        aria-hidden
        className={current ? "text-accent-text" : "text-fg-subtle"}
      />
      {!collapsed && <span className="min-w-0 flex-1 truncate text-left">{label}</span>}
    </button>
  );
}

const THEMES: { value: Theme; icon: LucideIcon }[] = [
  { value: "system", icon: SunMoon },
  { value: "light", icon: Sun },
  { value: "dark", icon: Moon },
  { value: "graphite", icon: Moon },
  { value: "hextech", icon: Moon },
];

function ThemeEntry({ collapsed }: { collapsed: boolean }) {
  const t = useT();
  const store = useStore();
  const current = useSettings().appearance.theme;
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const pick = (theme: Theme) => {
    setOpen(false);
    void store
      .updateSettings((settings) => ({
        ...settings,
        appearance: { ...settings.appearance, theme },
      }))
      .catch(() => undefined);
  };
  return (
    <div ref={anchor} className="relative">
      <Entry
        icon={THEMES.find((entry) => entry.value === current)?.icon ?? SunMoon}
        label={`${t("nav.theme")} · ${t(`settings.theme.${current}`)}`}
        collapsed={collapsed}
        popup="menu"
        expanded={open}
        onClick={() => setOpen((value) => !value)}
      />
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement={collapsed ? "right-end" : "top-start"}
        className="w-44"
      >
        <div role="menu" aria-label={t("nav.theme")}>
          {THEMES.map(({ value, icon: Icon }) => (
            <button
              key={value}
              type="button"
              role="menuitemradio"
              aria-checked={value === current}
              onClick={() => pick(value)}
              className="flex h-8 w-full items-center gap-2.5 rounded-6 px-2.5 text-[13px] text-fg hover-wash"
            >
              <Icon size={14} strokeWidth={2} aria-hidden className="text-fg-subtle" />
              <span className="flex-1 text-left">{t(`settings.theme.${value}`)}</span>
              {value === current && (
                <Check size={14} strokeWidth={2.25} aria-hidden className="text-accent-text" />
              )}
            </button>
          ))}
        </div>
      </Popover>
    </div>
  );
}

export function Sidebar({
  page,
  onNavigate,
  onSettings,
  collapsed,
  onToggleCollapsed,
  trafficLights,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  onSettings: () => void;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  trafficLights: boolean;
}) {
  const t = useT();
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const width = collapsed ? (trafficLights ? 76 : RAIL_WIDTH) : SIDEBAR_WIDTH;
  const CollapseIcon = collapsed ? PanelLeftOpen : PanelLeftClose;

  return (
    <nav
      aria-label={t("nav.label")}
      data-collapsed={collapsed}
      // A flex item's minimum is its content: all three are pinned so a label cannot widen the rail.
      style={{ width, minWidth: width, maxWidth: width }}
      className={cx(
        "flex h-full shrink-0 flex-col border-r border-border bg-nav pb-3",
        collapsed ? "px-2" : "px-3",
      )}
    >
      <div
        data-tauri-drag-region="deep"
        className={cx(
          "flex h-10 shrink-0 items-center gap-2",
          collapsed ? "justify-center" : "px-1.5",
        )}
        style={trafficLights && !collapsed ? { paddingLeft: TRAFFIC_LIGHTS_INSET - 12 } : undefined}
      >
        {!(trafficLights && collapsed) && <Logo size={20} className="shrink-0" />}
        {!collapsed && (
          <>
            <span className="text-[15px] font-semibold tracking-tight text-fg">winer</span>
            <Lamp
              tone={connected ? "ok" : "idle"}
              size={6}
              label={t(connected ? "connection.connected" : "connection.offline")}
            />
            <button
              type="button"
              aria-label={t("nav.collapse")}
              title={t("nav.collapse")}
              onClick={onToggleCollapsed}
              className="ml-auto grid size-7 place-items-center rounded-6 text-fg-subtle hover:text-fg hover-wash"
            >
              <CollapseIcon size={15} strokeWidth={2} />
            </button>
          </>
        )}
      </div>
      {collapsed && (
        <button
          type="button"
          aria-label={t("nav.expand")}
          title={t("nav.expand")}
          onClick={onToggleCollapsed}
          className="mx-auto mt-1 grid size-8 place-items-center rounded-6 text-fg-subtle hover:text-fg hover-wash"
        >
          <CollapseIcon size={15} strokeWidth={2} />
        </button>
      )}

      <div
        className={cx(
          "flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto",
          collapsed ? "mt-3" : "mt-5",
        )}
      >
        {NAV_GROUPS.map((group) => (
          <div key={group.title} role="group" aria-label={t(group.title)}>
            {collapsed ? (
              <div aria-hidden className="mx-2 mb-2 h-px bg-border" />
            ) : (
              <div className="mb-1 px-2.5 text-[11px] text-fg-subtle">{t(group.title)}</div>
            )}
            <ul className="flex flex-col gap-0.5">
              {group.items.map((item) => (
                <li key={item.page}>
                  <Entry
                    icon={item.icon}
                    label={t(item.label)}
                    collapsed={collapsed}
                    current={item.page === page}
                    onClick={() => onNavigate(item.page)}
                  />
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      <div className="mt-3 flex flex-col gap-0.5">
        <ThemeEntry collapsed={collapsed} />
        <Entry
          icon={Settings}
          label={t("nav.settings")}
          collapsed={collapsed}
          popup="dialog"
          onClick={onSettings}
        />
      </div>
    </nav>
  );
}
