// Settings › General: the build panel, and its data sources: where each mode's numbers come from,
// and the augment descriptions.
import type { ModeSource, RiftSource } from "@winer/shared";

import { errorMessage } from "../../lib/backend";
import { useT } from "../../lib/i18n";
import { useSettings, useStore } from "../../lib/store";
import { Row, Segmented, Toggle, toast } from "../../ui";

export function BuildSettingsRows() {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  const builds = settings.builds;
  const report = (error: unknown) => toast(errorMessage(error), "danger");
  const set = (change: Partial<typeof builds>) =>
    void store
      .updateSettings((value) => ({ ...value, builds: { ...value.builds, ...change } }))
      .catch(report);
  const setAugmentDetails = (augmentDetails: boolean) =>
    void store
      .updateSettings((value) => ({ ...value, general: { ...value.general, augmentDetails } }))
      .catch(report);
  const off = !builds.enabled;
  const modeOptions = [
    { value: "opGg" as const, label: t("loadout.source.opGg"), disabled: off },
    { value: "off" as const, label: t("loadout.source.off"), disabled: off },
  ];
  return (
    <>
      <Row label={t("loadout.builds")} help={t("loadout.buildsHint")}>
        <Toggle
          checked={builds.enabled}
          onChange={(enabled) => set({ enabled })}
          label={t("loadout.builds")}
        />
      </Row>
      <h3 className="eyebrow pt-5 pb-1">{t("loadout.sources")}</h3>
      <Row label={t("loadout.riftSource")} help={t("loadout.riftSourceHint")}>
        <Segmented<RiftSource>
          size="sm"
          label={t("loadout.riftSource")}
          value={builds.riftSource}
          options={[
            { value: "tencent", label: t("loadout.source.tencent"), disabled: off },
            { value: "opGg", label: t("loadout.source.opGg"), disabled: off },
          ]}
          onChange={(riftSource) => set({ riftSource })}
        />
      </Row>
      <Row label={t("loadout.aramSource")} help={t("loadout.opggSourceHint")}>
        <Segmented<ModeSource>
          size="sm"
          label={t("loadout.aramSource")}
          value={builds.aramSource}
          options={modeOptions}
          onChange={(aramSource) => set({ aramSource })}
        />
      </Row>
      <Row label={t("loadout.arenaSource")} help={t("loadout.opggSourceHint")}>
        <Segmented<ModeSource>
          size="sm"
          label={t("loadout.arenaSource")}
          value={builds.arenaSource}
          options={modeOptions}
          onChange={(arenaSource) => set({ arenaSource })}
        />
      </Row>
      <Row label={t("loadout.hextechFallback")} help={t("loadout.hextechFallbackHint")}>
        <Toggle
          checked={builds.hextechFallback}
          disabled={off}
          onChange={(hextechFallback) => set({ hextechFallback })}
          label={t("loadout.hextechFallback")}
        />
      </Row>
      <Row label={t("settings.augmentDetails")} help={t("settings.augmentDetailsHint")}>
        <Toggle
          checked={settings.general.augmentDetails}
          onChange={setAugmentDetails}
          label={t("settings.augmentDetails")}
        />
      </Row>
    </>
  );
}
