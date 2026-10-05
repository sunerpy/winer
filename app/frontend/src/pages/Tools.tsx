import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";

import { errorMessage } from "../lib/backend";
import { type MessageKey, useT } from "../lib/i18n";
import { useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { Button, Input, Panel, Row, Segmented, toast } from "../ui";
import { ConnectionGate, PageBody } from "./common";

/** The states the client offers itself. It sets `dnd` on its own during a game and ignores a request
 *  for it, and `mobile` is the phone app's, which the desktop client shows as 在线分组. */
const AVAILABILITY = ["chat", "away", "offline"] as const;
type Availability = (typeof AVAILABILITY)[number];

function isAvailability(value: string): value is Availability {
  return (AVAILABILITY as readonly string[]).includes(value);
}

function PresencePanel() {
  const t = useT();
  const store = useStore();
  const presence = useAsync(() => store.backend.call("get_presence"), []);
  /** What the client reports, which may be a state no option stands for (in game). */
  const [current, setCurrent] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!presence.data) return;
    setCurrent(presence.data.availability);
    setMessage(presence.data.statusMessage);
  }, [presence.data]);

  const changeAvailability = async (next: Availability) => {
    const previous = current;
    setCurrent(next);
    try {
      await store.backend.call("set_availability", { availability: next });
    } catch {
      setCurrent(previous);
      toast(t("tools.statusRefused"), "danger");
    }
  };

  const saveMessage = async () => {
    setSaving(true);
    try {
      await store.backend.call("set_status_message", { message });
      toast(t("common.saved"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Panel eyebrow={t("tools.social")} className="col-span-12">
      <Row label={t("tools.presence")} help={t("tools.presenceHint")}>
        {current !== null && !isAvailability(current) && (
          <span className="text-[11.5px] text-fg-subtle">
            {t("tools.current", { status: t(`tools.status.${current}` as MessageKey) })}
          </span>
        )}
        <Segmented
          size="sm"
          label={t("tools.presence")}
          value={current !== null && isAvailability(current) ? current : null}
          onChange={(next) => void changeAvailability(next)}
          options={AVAILABILITY.map((value) => ({
            value,
            label: t(`tools.status.${value}`),
            title: value === "offline" ? t("tools.status.offlineHint") : undefined,
          }))}
        />
      </Row>
      <Row label={t("tools.message")} help={t("tools.messageHint")} htmlFor="status-message">
        <Input
          id="status-message"
          value={message}
          maxLength={120}
          placeholder={t("tools.messagePlaceholder")}
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void saveMessage();
          }}
          className="w-[280px]"
        />
        <Button size="sm" loading={saving} onClick={() => void saveMessage()}>
          {t("common.save")}
        </Button>
      </Row>
    </Panel>
  );
}

function ClientPanel() {
  const t = useT();
  const store = useStore();
  const [busy, setBusy] = useState(false);
  const restart = async () => {
    setBusy(true);
    try {
      await store.backend.call("restart_client_ui");
      toast(t("tools.restarted"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };
  return (
    <Panel eyebrow={t("tools.client")} className="col-span-12">
      <Row label={t("tools.restartUi")} help={t("tools.restartUiHint")}>
        <Button size="sm" icon={RotateCcw} loading={busy} onClick={() => void restart()}>
          {t("tools.restartUi")}
        </Button>
      </Row>
    </Panel>
  );
}

export function ToolsPage() {
  const t = useT();
  return (
    <PageBody>
      <ConnectionGate offline={t("live.offline")}>
        <div className="grid grid-cols-12 gap-3">
          <PresencePanel />
          <ClientPanel />
        </div>
      </ConnectionGate>
    </PageBody>
  );
}
