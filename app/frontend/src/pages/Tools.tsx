import type { PresenceRule } from "@winer/shared";
import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";

import { errorMessage } from "../lib/backend";
import { type MessageKey, useT } from "../lib/i18n";
import { AVAILABILITIES, type Availability, isAvailability, namedStatus } from "../lib/presence";
import { useNotices, useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { Button, Input, Panel, Row, Segmented, Toggle, toast } from "../ui";
import { ConnectionGate, PageBody } from "./common";
import { BackgroundPanel } from "./tools/Background";
import { BackupPanel } from "./tools/Backup";
import { ChallengePanel } from "./tools/Challenges";
import { RankDisguisePanel } from "./tools/RankDisguise";

/** The states the client takes from winer. It sets `dnd` on its own during a game and ignores a
 *  request for it; `mobile` is the phone app's, which the desktop client keeps and shows as 在线分组. */
const HINTS: Partial<Record<Availability, MessageKey>> = {
  offline: "tools.status.offlineHint",
  mobile: "profile.mobileHint",
};

function PresencePanel() {
  const t = useT();
  const store = useStore();
  const rule = useSettings().profile.presence;
  const presence = useAsync(() => store.backend.call("get_presence"), []);
  /** What the client reports, which may be a state no option stands for (in game). */
  const [current, setCurrent] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  /** The message the client has, as opposed to the one being typed. */
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // winer put a remembered status back: show what the client says now.
  const restored = useNotices().find((notice) => notice.kind.kind === "presenceRestored")?.id;

  useEffect(() => {
    if (!presence.data) return;
    setCurrent(presence.data.availability);
    setMessage(presence.data.statusMessage);
    setSavedMessage(presence.data.statusMessage);
  }, [presence.data]);
  useEffect(() => {
    if (restored !== undefined) presence.reload();
  }, [restored, presence.reload]);

  const statusName = (value: string) => {
    const named = namedStatus(value);
    return named ? t(`tools.status.${named}` as MessageKey) : value;
  };

  /** Keeps what winer puts back in step with what the user just set, while it is remembered. */
  const remember = (change: Partial<PresenceRule>) => {
    if (!rule.remember) return;
    void store
      .updateSettings((settings) => ({
        ...settings,
        profile: {
          ...settings.profile,
          presence: { ...settings.profile.presence, ...change },
        },
      }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  };

  const changeAvailability = async (next: Availability) => {
    const previous = current;
    setCurrent(next);
    try {
      await store.backend.call("set_availability", { availability: next });
      remember({ availability: next });
    } catch {
      setCurrent(previous);
      toast(t("tools.statusRefused"), "danger");
    }
  };

  const saveMessage = async () => {
    setSaving(true);
    try {
      await store.backend.call("set_status_message", { message });
      setSavedMessage(message);
      remember({ statusMessage: message });
      toast(t("common.saved"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setSaving(false);
    }
  };

  /** Switched on, the status and message on screen are the ones kept. */
  const switchRemember = (on: boolean) =>
    void store
      .updateSettings((settings) => {
        const kept = settings.profile.presence;
        return {
          ...settings,
          profile: {
            ...settings.profile,
            presence: on
              ? {
                  remember: true,
                  availability: isAvailability(current) ? current : kept.availability,
                  statusMessage: savedMessage ?? kept.statusMessage,
                }
              : { ...kept, remember: false },
          },
        };
      })
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  return (
    <Panel eyebrow={t("tools.social")} className="col-span-12">
      <Row label={t("tools.presence")} help={t("tools.presenceHint")}>
        {current !== null && !isAvailability(current) && (
          <span className="text-[11.5px] text-fg-subtle">
            {t("tools.current", { status: statusName(current) })}
          </span>
        )}
        <Segmented
          size="sm"
          label={t("tools.presence")}
          value={isAvailability(current) ? current : null}
          onChange={(next) => void changeAvailability(next)}
          options={AVAILABILITIES.map((value) => {
            const hint = HINTS[value];
            return {
              value,
              label: t(`tools.status.${value}`),
              title: hint ? t(hint) : undefined,
            };
          })}
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
      <Row label={t("profile.remember")} help={t("profile.rememberHint")}>
        <Toggle checked={rule.remember} onChange={switchRemember} label={t("profile.remember")} />
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
          <BackgroundPanel />
          <ChallengePanel />
          <RankDisguisePanel />
          <BackupPanel />
        </div>
      </ConnectionGate>
    </PageBody>
  );
}
