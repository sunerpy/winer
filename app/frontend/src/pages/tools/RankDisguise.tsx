// The rank friends see in the friends list and on the hover card. Only the chat presence changes;
// the switch says so, and it starts off.
import type { DisguiseQueue, Division, RankDisguise, Tier } from "@winer/shared";
import { TIER_COLORS, tierLabel } from "@winer/shared";

import { errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { useLanguage, useT } from "../../lib/i18n";
import { useSettings, useStore } from "../../lib/store";
import { Panel, Row, Segmented, Toggle, onRadioKeys, toast } from "../../ui";
import { DIVISIONS, TIERS, hasDivisions } from "./profile";

/** Ten tiers do not fit one track: a radio group of chips that wraps, each with its tier's dot. */
function TierChoice({
  value,
  onChange,
  label,
}: {
  value: Tier;
  onChange: (tier: Tier) => void;
  label: string;
}) {
  const language = useLanguage();
  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={(event) => onRadioKeys(event, TIERS, value, onChange)}
      className="flex flex-wrap justify-end gap-1"
    >
      {TIERS.map((tier) => {
        const selected = tier === value;
        return (
          <button
            key={tier}
            type="button"
            role="radio"
            data-value={tier}
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(tier)}
            className={cx(
              "flex h-6 items-center gap-1.5 rounded-pill px-2.5 text-[11.5px] transition-colors duration-150 hairline",
              selected
                ? "bg-accent-soft font-medium text-accent-text"
                : "text-fg-muted hover:text-fg",
            )}
          >
            <span
              aria-hidden
              className="size-1.5 rounded-full"
              style={{ backgroundColor: TIER_COLORS[tier] }}
            />
            {tierLabel(tier, language, true)}
          </button>
        );
      })}
    </div>
  );
}

export function RankDisguisePanel() {
  const t = useT();
  const language = useLanguage();
  const store = useStore();
  const rule = useSettings().profile.rankDisguise;
  const save = (change: (rule: RankDisguise) => RankDisguise) =>
    void store
      .updateSettings((settings) => ({
        ...settings,
        profile: { ...settings.profile, rankDisguise: change(settings.profile.rankDisguise) },
      }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  const queueLabel = (queue: DisguiseQueue) =>
    queue === "solo" ? t("common.solo") : t("common.flex");
  const rank = hasDivisions(rule.tier)
    ? `${tierLabel(rule.tier, language)} ${rule.division}`
    : tierLabel(rule.tier, language);

  return (
    <Panel eyebrow={t("profile.rank.title")} className="col-span-12 @[900px]:col-span-6">
      <Row label={t("profile.rank.enable")} help={t("profile.rank.hint")}>
        <Toggle
          checked={rule.enabled}
          onChange={(enabled) => save((value) => ({ ...value, enabled }))}
          label={t("profile.rank.enable")}
        />
      </Row>
      <Row label={t("profile.rank.queue")}>
        <Segmented<DisguiseQueue>
          size="sm"
          label={t("profile.rank.queue")}
          value={rule.queue}
          options={(["solo", "flex"] as const).map((queue) => ({
            value: queue,
            label: queueLabel(queue),
          }))}
          onChange={(queue) => save((value) => ({ ...value, queue }))}
        />
      </Row>
      <Row label={t("profile.rank.tier")}>
        <TierChoice
          label={t("profile.rank.tier")}
          value={rule.tier}
          onChange={(tier) => save((value) => ({ ...value, tier }))}
        />
      </Row>
      {hasDivisions(rule.tier) && (
        <Row label={t("profile.rank.division")}>
          <Segmented<Division>
            size="sm"
            label={t("profile.rank.division")}
            value={rule.division}
            options={DIVISIONS.map((division) => ({ value: division, label: division }))}
            onChange={(division) => save((value) => ({ ...value, division }))}
          />
        </Row>
      )}
      <p className="pt-3 text-[12px] text-fg-muted">
        {rule.enabled
          ? t("profile.rank.preview", { rank, queue: queueLabel(rule.queue) })
          : t("profile.rank.offPreview")}
      </p>
    </Panel>
  );
}
