// The 28px line under the content: where the client connection stands, and the shortcuts.
import { useT } from "../lib/i18n";
import { useLive } from "../lib/store";
import { Lamp } from "../ui";

export function StatusBar() {
  const t = useT();
  const connection = useLive((snapshot) => snapshot.connection);
  const phase = useLive((snapshot) => snapshot.phase);
  const status = connection.status;
  return (
    <footer className="mono flex h-7 shrink-0 items-center gap-3 overflow-hidden border-t border-border bg-surface px-6 text-[11px] whitespace-nowrap text-fg-subtle">
      <span className="flex items-center gap-1.5">
        <Lamp
          tone={status === "connected" ? "ok" : status === "accessDenied" ? "danger" : "idle"}
          size={6}
          pulse={status === "connecting"}
        />
        {status === "connected"
          ? `${t("connection.connected")} · ${connection.platformId || "LCU"} · :${connection.port}`
          : status === "accessDenied"
            ? t("connection.accessDenied")
            : status === "connecting"
              ? t("connection.connecting")
              : t("connection.searching")}
      </span>
      {status === "connected" && (
        <>
          <span aria-hidden>·</span>
          <span>{t(`phase.${phase}`)}</span>
        </>
      )}
      <span className="ml-auto hidden lg:inline">{t("status.shortcuts")}</span>
    </footer>
  );
}
