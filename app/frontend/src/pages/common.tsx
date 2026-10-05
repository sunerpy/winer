// Pieces every page uses: the page frame and what to show while the client is not connected.
import { PlugZap, ShieldAlert } from "lucide-react";
import type { ReactNode } from "react";

import { errorMessage } from "../lib/backend";
import { cx } from "../lib/cx";
import { useT } from "../lib/i18n";
import { useLive, useStore } from "../lib/store";
import { Button, EmptyState, Lamp, toast } from "../ui";

export function PageBody({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cx("@container mx-auto w-full max-w-[1280px] px-6 pt-5 pb-8", className)}>
      {children}
    </div>
  );
}

function useRelaunch() {
  const store = useStore();
  return () =>
    void store.backend
      .call("relaunch_elevated")
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
}

/** The connection, as one sentence with a lamp, and what to do about it. */
export function ConnectionBanner({ actions }: { actions?: ReactNode }) {
  const t = useT();
  const relaunch = useRelaunch();
  const connection = useLive((snapshot) => snapshot.connection);
  const phase = useLive((snapshot) => snapshot.phase);

  const [tone, title, hint] =
    connection.status === "connected"
      ? ([
          "ok",
          `${t("connection.connected")} · ${t(`phase.${phase}`)}`,
          connection.platformId,
        ] as const)
      : connection.status === "accessDenied"
        ? (["danger", t("connection.accessDenied"), t("connection.accessDeniedHint")] as const)
        : connection.status === "connecting"
          ? (["warn", t("connection.connecting"), t("connection.connectingHint")] as const)
          : (["idle", t("connection.searching"), t("connection.searchingHint")] as const);

  return (
    <div
      role="status"
      className={cx(
        "flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 rounded-10 px-4 py-2.5 hairline",
        connection.status === "accessDenied" ? "bg-danger-soft" : "bg-surface",
      )}
    >
      <Lamp
        tone={tone}
        size={10}
        pulse={connection.status === "connecting" || connection.status === "searching"}
      />
      <p className="text-[15px] font-semibold text-fg">{title}</p>
      {hint && <p className="min-w-0 flex-1 truncate text-[12.5px] text-fg-muted">{hint}</p>}
      <div className="ml-auto flex items-center gap-2">
        {connection.status === "accessDenied" && (
          <Button variant="primary" size="sm" icon={ShieldAlert} onClick={relaunch}>
            {t("connection.relaunch")}
          </Button>
        )}
        {actions}
      </div>
    </div>
  );
}

/** Renders `children` only while the client is connected; otherwise says why there is nothing. */
export function ConnectionGate({ offline, children }: { offline: string; children: ReactNode }) {
  const t = useT();
  const relaunch = useRelaunch();
  const status = useLive((snapshot) => snapshot.connection.status);
  if (status === "connected") return children;
  return (
    <EmptyState
      icon={status === "accessDenied" ? ShieldAlert : PlugZap}
      title={
        status === "accessDenied"
          ? t("connection.accessDenied")
          : status === "connecting"
            ? t("connection.connecting")
            : t("connection.searching")
      }
      actions={
        status === "accessDenied" ? (
          <Button variant="primary" icon={ShieldAlert} onClick={relaunch}>
            {t("connection.relaunch")}
          </Button>
        ) : undefined
      }
    >
      {status === "accessDenied" ? t("connection.accessDeniedHint") : offline}
    </EmptyState>
  );
}
