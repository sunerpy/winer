// The Automation page's runes and spells and its item sets: each a rule card like the others, with
// the user's own setups (我的方案), the setups an older winer remembered, and a way to take winer's
// own changes back out.
import type { LoadoutRule } from "@winer/shared";
import { Eraser, Trash2 } from "lucide-react";
import { type ReactNode, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { useT } from "../../lib/i18n";
import { useLive, useSettings, useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import { Badge, Button, Panel, Row, Toggle, toast } from "../../ui";

export function LoadoutPanel({
  eyebrow,
  rule,
  onChange,
  scope,
}: {
  eyebrow: string;
  rule: LoadoutRule;
  onChange: (change: (rule: LoadoutRule) => LoadoutRule) => void;
  scope: ReactNode;
}) {
  const t = useT();
  const store = useStore();
  // A setup saved in champ select is one more: champ select's view moving is when to count again.
  const revision = useLive((snapshot) => snapshot.champSelect?.loadout?.revision ?? 0);
  const phase = useLive((snapshot) => snapshot.phase);
  const summary = useAsync(() => store.backend.call("get_loadout_summary"), [phase, revision]);
  const [busy, setBusy] = useState<"presets" | "legacy" | null>(null);
  const presets = summary.data?.presets ?? 0;
  const legacy = summary.data?.legacy ?? null;
  const clear = async (what: "presets" | "legacy") => {
    setBusy(what);
    try {
      await store.backend.call(what === "presets" ? "clear_loadout_presets" : "clear_loadouts");
      summary.reload();
      toast(t(what === "presets" ? "loadout.forgotten" : "loadout.legacyDeleted"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };
  return (
    <Panel eyebrow={eyebrow} className="col-span-12">
      <Row label={t("loadout.rule")} help={t("loadout.ruleHint")}>
        <Toggle
          checked={rule.enabled}
          onChange={(enabled) => onChange((value) => ({ ...value, enabled }))}
          label={t("loadout.rule")}
        />
      </Row>
      {scope}
      <Row label={t("loadout.recommended")} help={t("loadout.recommendedHint")}>
        <Toggle
          checked={rule.recommended}
          onChange={(recommended) => onChange((value) => ({ ...value, recommended }))}
          label={t("loadout.recommended")}
        />
      </Row>
      <Row label={t("loadout.presets", { n: presets })} help={t("loadout.presetsHint")}>
        <Button
          size="sm"
          variant="ghost"
          icon={Eraser}
          loading={busy === "presets"}
          disabled={presets === 0 || busy !== null}
          onClick={() => void clear("presets")}
        >
          {t("loadout.forget")}
        </Button>
      </Row>
      {legacy !== null && (
        <Row
          label={t("loadout.legacy")}
          help={
            legacy > 0 ? t("loadout.legacyCount", { n: legacy }) : t("loadout.legacyUnreadable")
          }
        >
          <Button
            size="sm"
            variant="ghost"
            icon={Trash2}
            loading={busy === "legacy"}
            disabled={busy !== null}
            onClick={() => void clear("legacy")}
          >
            {t("loadout.legacyDelete")}
          </Button>
        </Row>
      )}
      <p className="pt-3 text-[12px] leading-4 text-fg-muted">{t("loadout.pageNote")}</p>
    </Panel>
  );
}

export function ItemSetsPanel({
  eyebrow,
  enabled,
  onChange,
  scope,
}: {
  eyebrow: string;
  enabled: boolean;
  onChange: (enabled: boolean) => void;
  scope: ReactNode;
}) {
  const t = useT();
  const store = useStore();
  const builds = useSettings().builds.enabled;
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const [busy, setBusy] = useState(false);
  const clear = async () => {
    setBusy(true);
    try {
      const removed = await store.backend.call("clear_item_sets");
      toast(t("loadout.clearedItemSets", { n: removed }), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };
  return (
    <Panel
      eyebrow={eyebrow}
      right={<Badge tone="warning">{t("loadout.experimental")}</Badge>}
      className="col-span-12"
    >
      <Row label={t("loadout.itemSets")} help={t("loadout.itemSetsHint")}>
        <Toggle checked={enabled} onChange={onChange} label={t("loadout.itemSets")} />
      </Row>
      {scope}
      <Row
        label={t("loadout.clearItemSets")}
        help={builds ? t("loadout.itemSetsNote") : t("loadout.itemSetsNeedBuilds")}
      >
        <Button
          size="sm"
          variant="ghost"
          icon={Trash2}
          loading={busy}
          disabled={!connected}
          onClick={() => void clear()}
        >
          {t("loadout.clearItemSets")}
        </Button>
      </Row>
    </Panel>
  );
}
