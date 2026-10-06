import type { PluginSettings, PluginStatus } from "@winer/shared";
import { Power, PowerOff, RefreshCw, RotateCcw } from "lucide-react";
import { useState } from "react";

import { errorMessage } from "../lib/backend";
import { useT } from "../lib/i18n";
import { useLive, useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { Badge, Button, Card, Input, Lamp, Panel, Row, Skeleton, Toggle, toast } from "../ui";
import { PageBody } from "./common";

function StatusPanel({
  status,
  loading,
  onChange,
  onRefresh,
}: {
  status: PluginStatus | undefined;
  loading: boolean;
  onChange: (status: PluginStatus) => void;
  onRefresh: () => void;
}) {
  const t = useT();
  const store = useStore();
  const auto = useSettings().plugin.auto;
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const [busy, setBusy] = useState<"enable" | "disable" | "reload" | null>(null);

  const run = async (action: "enable" | "disable" | "reload") => {
    setBusy(action);
    try {
      if (action === "reload") {
        await store.backend.call("restart_client_ui");
        toast(t("tools.restarted"), "ok");
      } else {
        onChange(
          await store.backend.call(action === "enable" ? "enable_plugin" : "disable_plugin"),
        );
        toast(t(action === "enable" ? "plugin.enabled" : "plugin.disabled"), "ok");
      }
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };

  if (!status) {
    return (
      <Card className="col-span-12 flex flex-col gap-3">
        {loading ? (
          <Skeleton className="h-5 w-64" />
        ) : (
          <p className="text-[13px] text-fg-muted">{t("common.noData")}</p>
        )}
      </Card>
    );
  }

  // What the loader row says, worst first: a stated problem, then off, then not there yet.
  const [tone, state] = status.active
    ? ([
        "ok",
        status.managed
          ? t("plugin.loaderManaged", { version: status.bundledLoader })
          : t("plugin.loaderOwn"),
      ] as const)
    : !auto
      ? (["off", t("plugin.loaderOff")] as const)
      : status.occupied
        ? (["warn", t("plugin.loaderOccupied")] as const)
        : status.setupError
          ? (["danger", t("plugin.loaderFailed", { error: status.setupError })] as const)
          : (["idle", t("plugin.loaderWaiting")] as const);
  return (
    <Panel
      eyebrow={t("plugin.setup")}
      className="col-span-12"
      right={
        <Button size="sm" variant="ghost" icon={RefreshCw} onClick={onRefresh}>
          {t("plugin.refresh")}
        </Button>
      }
    >
      <p className="mb-2 max-w-[720px] text-[12.5px] leading-5 text-fg-muted">
        {t("plugin.about")}
      </p>
      <Row label={t("plugin.loader")} help={status.loaderDir ?? t("plugin.loaderWhere")}>
        <span className="flex items-center gap-2 text-[12.5px]">
          <Lamp tone={tone} />
          {state}
        </span>
      </Row>
      <Row
        label={t("plugin.installed")}
        help={t("plugin.bundled", { version: status.bundledVersion })}
      >
        {status.installedVersion ? (
          <Badge tone={status.current ? "ok" : "warning"}>{status.installedVersion}</Badge>
        ) : (
          <Badge>{t("plugin.notInstalled")}</Badge>
        )}
        <span className="flex items-center gap-1.5 text-[12px] text-fg-muted">
          <Lamp tone={status.connected > 0 ? "ok" : "off"} size={6} />
          {status.connected > 0
            ? t("plugin.connected", { n: status.connected })
            : t("plugin.disconnected")}
        </span>
      </Row>
      <div className="flex flex-wrap items-center gap-2 pt-3">
        {auto ? (
          <>
            <Button
              icon={RotateCcw}
              disabled={!connected}
              loading={busy === "reload"}
              onClick={() => void run("reload")}
            >
              {t("plugin.reload")}
            </Button>
            <Button
              variant="ghost"
              icon={PowerOff}
              loading={busy === "disable"}
              onClick={() => void run("disable")}
            >
              {t("plugin.disable")}
            </Button>
          </>
        ) : (
          <Button
            variant="accent"
            icon={Power}
            loading={busy === "enable"}
            onClick={() => void run("enable")}
          >
            {t("plugin.enable")}
          </Button>
        )}
        <span className="text-[11.5px] text-fg-subtle">{t("plugin.reloadHint")}</span>
      </div>
    </Panel>
  );
}

function FeaturesPanel() {
  const t = useT();
  const store = useStore();
  const plugin = useSettings().plugin;
  const [dir, setDir] = useState(plugin.loaderDir ?? "");
  const save = (change: (plugin: PluginSettings) => PluginSettings) =>
    void store
      .updateSettings((settings) => ({ ...settings, plugin: change(settings.plugin) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  return (
    <Panel eyebrow={t("plugin.features")} className="col-span-12">
      <Row label={t("plugin.teamPanel")} help={t("plugin.teamPanelHint")}>
        <Toggle
          checked={plugin.teamPanel}
          onChange={(teamPanel) => save((value) => ({ ...value, teamPanel }))}
          label={t("plugin.teamPanel")}
        />
      </Row>
      <Row label={t("plugin.benchNoCooldown")} help={t("plugin.benchNoCooldownHint")}>
        <Toggle
          checked={plugin.benchNoCooldown}
          onChange={(benchNoCooldown) => save((value) => ({ ...value, benchNoCooldown }))}
          label={t("plugin.benchNoCooldown")}
        />
      </Row>
      <Row label={t("social.pluginFriends")} help={t("social.pluginFriendsHint")}>
        <Toggle
          checked={plugin.friendStatus}
          onChange={(friendStatus) => save((value) => ({ ...value, friendStatus }))}
          label={t("social.pluginFriends")}
        />
      </Row>
      <Row label={t("social.pluginLobby")} help={t("social.pluginLobbyHint")}>
        <Toggle
          checked={plugin.lobbyPanel}
          onChange={(lobbyPanel) => save((value) => ({ ...value, lobbyPanel }))}
          label={t("social.pluginLobby")}
        />
      </Row>
      <Row label={t("plugin.hidePromotions")} help={t("plugin.hidePromotionsHint")}>
        <Toggle
          checked={plugin.hidePromotions}
          onChange={(hidePromotions) => save((value) => ({ ...value, hidePromotions }))}
          label={t("plugin.hidePromotions")}
        />
      </Row>
      <Row label={t("plugin.dir")} help={t("plugin.dirHint")} htmlFor="loader-dir">
        <Input
          id="loader-dir"
          value={dir}
          onChange={(event) => setDir(event.target.value)}
          placeholder={"C:\\Pengu Loader"}
          className="w-[280px]"
        />
        <Button
          size="sm"
          disabled={dir === (plugin.loaderDir ?? "")}
          onClick={() => save((value) => ({ ...value, loaderDir: dir.trim() || null }))}
        >
          {t("common.save")}
        </Button>
      </Row>
    </Panel>
  );
}

export function PluginPage() {
  const store = useStore();
  const settings = useSettings();
  const query = useAsync(
    () => store.backend.call("get_plugin_status"),
    [settings.plugin.loaderDir],
  );
  const [override, setOverride] = useState<PluginStatus | undefined>();
  const status = override ?? query.data;
  return (
    <PageBody className="grid grid-cols-12 gap-3">
      <StatusPanel
        status={status}
        loading={query.loading}
        onChange={setOverride}
        onRefresh={() => {
          setOverride(undefined);
          query.reload();
        }}
      />
      <FeaturesPanel />
    </PageBody>
  );
}
