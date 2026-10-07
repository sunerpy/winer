// Sidebar 224 + title bar 40 + content + status bar 28, plus the settings dialog, the update dialog
// and the toasts. Only <main> scrolls.
import { riotId } from "@winer/shared";
import { ArrowUpCircle } from "lucide-react";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { useT } from "../lib/i18n";
import { useObservable } from "../lib/observable";
import { useLive, useStore, useUpdateStatus } from "../lib/store";
import { useWindowChrome } from "../lib/window";
import { AutomationPage } from "../pages/Automation";
import { HistoryPage } from "../pages/History";
import { LivePage } from "../pages/Live";
import { OverviewPage } from "../pages/Overview";
import { PluginPage } from "../pages/Plugin";
import { ToolsPage } from "../pages/Tools";
import { SettingsDialog } from "../pages/settings/SettingsDialog";
import { UpdateDialog } from "../pages/settings/UpdateDialog";
import { Lamp, Toaster } from "../ui";
import { ErrorBoundary } from "./ErrorBoundary";
import {
  PAGE_ORDER,
  type Page,
  type Route,
  type SettingsSection,
  ShellContext,
} from "./navigation";
import { useNoticeToasts } from "./notices";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { TitleBar } from "./TitleBar";

const COLLAPSED_KEY = "winer.sidebar.collapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
}

function Readout() {
  const t = useT();
  const me = useLive((snapshot) => snapshot.me);
  const phase = useLive((snapshot) => snapshot.phase);
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  if (!connected || !me) return null;
  return (
    <>
      <span className="truncate">{riotId(me.name)}</span>
      <span aria-hidden>·</span>
      <Lamp
        tone={phase === "None" || phase === "Lobby" ? "idle" : "accent"}
        size={6}
        pulse={phase === "Matchmaking" || phase === "ReadyCheck"}
      />
      <span>{t(`phase.${phase}`)}</span>
    </>
  );
}

function UpdateBadge({ onOpen }: { onOpen: () => void }) {
  const t = useT();
  const status = useUpdateStatus();
  if (status.state !== "available" && status.state !== "downloading" && status.state !== "ready")
    return null;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="inline-flex h-7 items-center gap-1.5 rounded-6 bg-accent-soft px-2 text-[12px] font-medium text-accent-text hover:brightness-110"
    >
      <ArrowUpCircle size={14} strokeWidth={2} aria-hidden />
      {status.state === "available"
        ? t("update.available", { version: status.version })
        : status.state === "downloading"
          ? t("update.downloading", { version: status.version })
          : t("update.restartBadge", { version: status.version })}
    </button>
  );
}

const TITLES: Record<Page, Parameters<ReturnType<typeof useT>>[0]> = {
  overview: "overview.title",
  live: "live.title",
  history: "history.title",
  automation: "nav.automation",
  tools: "tools.title",
  plugin: "plugin.title",
};

function PageView({ route }: { route: Route }): ReactNode {
  switch (route.page) {
    case "overview":
      return <OverviewPage />;
    case "live":
      return <LivePage />;
    case "history":
      return <HistoryPage puuid={route.puuid} />;
    case "automation":
      return <AutomationPage />;
    case "tools":
      return <ToolsPage />;
    case "plugin":
      return <PluginPage />;
  }
}

export function Shell() {
  const t = useT();
  const store = useStore();
  const chrome = useWindowChrome();
  const [route, setRoute] = useState<Route>({ page: "overview" });
  const [settings, setSettings] = useState<SettingsSection | null>(null);
  const [updateOpen, setUpdateOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const main = useRef<HTMLElement>(null);
  useNoticeToasts();

  // A lobby member clicked in the client: the shell has raised the window, the history opens.
  const historyRequest = useObservable(store.historyRequest);
  useEffect(() => {
    if (!historyRequest) return;
    setSettings(null);
    setRoute({ page: "history", puuid: historyRequest.puuid });
  }, [historyRequest]);

  // Every page scrolls in the one <main>: each opens at its top, not where the last was left.
  useLayoutEffect(() => {
    if (main.current) main.current.scrollTop = 0;
  }, [route]);

  const toggleCollapsed = useCallback(() => {
    setCollapsed((value) => {
      try {
        localStorage.setItem(COLLAPSED_KEY, String(!value));
      } catch {
        // Layout state only; losing it costs one click.
      }
      return !value;
    });
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
      const index = Number(event.key) - 1;
      const page = PAGE_ORDER[index];
      if (page) {
        event.preventDefault();
        setRoute({ page });
      } else if (event.key === ",") {
        event.preventDefault();
        setSettings("appearance");
      } else if (event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleCollapsed();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleCollapsed]);

  const shell = useMemo(
    () => ({
      route,
      navigate: setRoute,
      openSettings: (section?: SettingsSection) => setSettings(section ?? "appearance"),
    }),
    [route],
  );

  return (
    <ShellContext value={shell}>
      <div className="flex h-full min-h-0 bg-canvas text-fg">
        <Sidebar
          page={route.page}
          onNavigate={(page) => setRoute({ page })}
          onSettings={() => setSettings("appearance")}
          collapsed={collapsed}
          onToggleCollapsed={toggleCollapsed}
          trafficLights={chrome.platform === "macos"}
        />
        <div className="flex min-w-0 flex-1 flex-col">
          <TitleBar
            title={t(TITLES[route.page])}
            readout={<Readout />}
            right={<UpdateBadge onOpen={() => setUpdateOpen(true)} />}
            chrome={chrome}
          />
          <main ref={main} className="min-h-0 flex-1 overflow-y-auto">
            <ErrorBoundary
              key={route.page}
              title={t("error.title")}
              hint={t("error.hint")}
              retryLabel={t("common.retry")}
              logsLabel={t("settings.openLogs")}
              onOpenLogs={() => void store.backend.call("reveal_logs").catch(() => undefined)}
            >
              <PageView route={route} />
            </ErrorBoundary>
          </main>
          <StatusBar />
        </div>
      </div>
      <SettingsDialog
        section={settings}
        onSection={setSettings}
        onClose={() => setSettings(null)}
        onOpenUpdate={() => setUpdateOpen(true)}
      />
      <UpdateDialog open={updateOpen} onClose={() => setUpdateOpen(false)} />
      <Toaster closeLabel={t("common.close")} />
    </ShellContext>
  );
}
