// What a player's numbers count and how their tier came about, said where the numbers are: a
// summary like `10胜8负 · 56%` otherwise reads as ranked solo/duo.
import {
  GRADE_LETTERS,
  type FormScope,
  type Mode,
  type PlayerStanding,
  type RecentForm,
  type TierSet,
} from "@winer/shared";

import { type MessageKey, useT } from "../lib/i18n";
import { MODE_LABEL, MODES, modeOf } from "../lib/modes";
import { useCatalog, useSettings } from "../lib/store";
import { Hint } from "../ui";

/** The counted games by kind of game, the most played first. */
export function formModes(
  form: RecentForm,
  catalog: ReturnType<typeof useCatalog>,
): { mode: Mode; n: number }[] {
  const counts = new Map<Mode, number>();
  for (const game of form.matches) {
    if (game.remake) continue;
    const queue = catalog?.queues.get(game.queueId);
    const mode = queue ? modeOf(queue.gameMode, queue.ranked) : "other";
    counts.set(mode, (counts.get(mode) ?? 0) + 1);
  }
  return MODES.flatMap((mode) => {
    const n = counts.get(mode);
    return n ? [{ mode, n }] : [];
  }).sort((a, b) => b.n - a.n);
}

/** `近 18 场 · 所有模式` and the rule behind it. */
export function FormLabel({ form, scope }: { form: RecentForm; scope: FormScope | null }) {
  const t = useT();
  const catalog = useCatalog();
  const modes = formModes(form, catalog);
  return (
    <span className="inline-flex items-center gap-1">
      <span>{t("history.formLabel", { n: form.games })}</span>
      <Hint label={t("history.formHint")} title={t("history.formHint")}>
        <p>
          {scope
            ? t("history.formWindow", { listed: scope.listed, window: form.matches.length })
            : t("history.formWindowShort", { window: form.matches.length })}
        </p>
        {modes.length > 0 && (
          <p>
            {t("history.formModes", {
              n: form.games,
              modes: modes
                .map(({ mode, n }) => t("history.formMode", { mode: t(MODE_LABEL[mode]), n }))
                .join(t("history.formModeJoin")),
            })}
          </p>
        )}
        {(scope?.remakes ?? 0) > 0 && <p>{t("history.formRemakes", { n: scope?.remakes ?? 0 })}</p>}
        <p>
          {scope
            ? t("history.formLeftOut", { custom: scope.custom, bots: scope.bots })
            : t("history.formLeftOutShort")}
        </p>
        <p>{t("history.formSame")}</p>
      </Hint>
    </span>
  );
}

const SCHEME: Record<TierSet, MessageKey> = {
  riftFive: "rating.scheme.riftFive",
  grades: "rating.scheme.grades",
  horseUniverse: "rating.scheme.horseUniverse",
  horsesFive: "rating.scheme.horsesFive",
  horses: "rating.scheme.horses",
  rift: "rating.scheme.rift",
  custom: "rating.scheme.custom",
};

/** How a player alone came to their tier: champ select's rule, and how it is read for one. */
export function StandingRule({ standing, form }: { standing: PlayerStanding; form: RecentForm }) {
  const t = useT();
  const rule = useSettings().automation.callout;
  const rating = standing.rating;
  if (!rating || standing.band === null) return null;
  const grade = GRADE_LETTERS[standing.band] ?? "";
  // Fewer than two names of the user's own rank like 赛马三档 (callout::ranking).
  const scheme =
    rule.tiers === "custom" && rule.customTiers.filter((name) => name.trim()).length < 2
      ? t("rating.scheme.horses")
      : t(SCHEME[rule.tiers]);
  return (
    <Hint label={t("history.standingHint")} title={t("history.standingHint")}>
      <p>{t("history.standingScore", { score: rating.score.toFixed(1) })}</p>
      {form.source && (
        <p>{t(form.source === "full" ? "history.standingFull" : "history.standingLite")}</p>
      )}
      {form.away > 0 && <p>{t("history.standingAway", { n: form.away })}</p>}
      <p>
        {rating.grade !== null
          ? t("history.standingGraded", { grade, label: rating.label })
          : t("history.standingAlone", {
              scheme,
              grade,
              tiers: rating.tiers,
              label: rating.label,
            })}
      </p>
      <p>{t("history.standingWords")}</p>
    </Hint>
  );
}
