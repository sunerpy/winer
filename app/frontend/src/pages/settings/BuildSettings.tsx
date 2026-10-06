// Settings › General: the build panel, and where Summoner's Rift numbers come from.
import type { RiftSource } from "@winer/shared";

import { errorMessage } from "../../lib/backend";
import { useT } from "../../lib/i18n";
import { useSettings, useStore } from "../../lib/store";
import { Row, Segmented, Toggle, toast } from "../../ui";

export function BuildSettingsRows() {
  const t = useT();
  const store = useStore();
  const builds = useSettings().builds;
  const set = (change: Partial<typeof builds>) =>
    void store
      .updateSettings((settings) => ({ ...settings, builds: { ...settings.builds, ...change } }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  return (
    <>
      <Row label={t("loadout.builds")} help={t("loadout.buildsHint")}>
        <Toggle
          checked={builds.enabled}
          onChange={(enabled) => set({ enabled })}
          label={t("loadout.builds")}
        />
      </Row>
      <Row label={t("loadout.riftSource")} help={t("loadout.riftSourceHint")}>
        <Segmented<RiftSource>
          size="sm"
          label={t("loadout.riftSource")}
          value={builds.riftSource}
          options={[
            { value: "tencent", label: t("loadout.source.tencent"), disabled: !builds.enabled },
            { value: "opGg", label: t("loadout.source.opGg"), disabled: !builds.enabled },
          ]}
          onChange={(riftSource) => set({ riftSource })}
        />
      </Row>
    </>
  );
}
