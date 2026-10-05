// One game in a history list, from the player's point of view.
import { type Award, compact, gameLength, relativeTime, type MatchSummary } from "@winer/shared";

import { cx } from "../lib/cx";
import { useLanguage, useT } from "../lib/i18n";
import { useCatalog } from "../lib/store";
import { FeatChips } from "./Feats";
import { AssetIcon, AugmentIcon, ChampionIcon } from "./icons";
import { KdaValue } from "./stats";

export function queueName(
  queueId: number,
  gameMode: string,
  catalog: ReturnType<typeof useCatalog>,
  fallback: string,
): string {
  return catalog?.queues.get(queueId)?.name || gameMode || fallback;
}

/** MVP or SVP on the champion's corner: solid both, since a translucent chip on artwork cannot be
 *  read. */
function AwardMark({ award }: { award: Award }) {
  const t = useT();
  return (
    <span
      title={t(award === "mvp" ? "history.awardMvp" : "history.awardSvp")}
      className={cx(
        "absolute -top-1.5 -left-1.5 rounded-4 px-1 text-[9.5px] leading-3.5 font-bold tracking-wide",
        award === "mvp" ? "bg-accent text-accent-fg" : "bg-raised text-fg hairline",
      )}
    >
      {award === "mvp" ? "MVP" : "SVP"}
    </span>
  );
}

export function MatchRow({
  game,
  selected = false,
  onSelect,
  dense = false,
  now = Date.now(),
}: {
  game: MatchSummary;
  selected?: boolean;
  onSelect?: () => void;
  /** Overview's short list: no item row. */
  dense?: boolean;
  now?: number;
}) {
  const t = useT();
  const language = useLanguage();
  const catalog = useCatalog();
  const line = game.line;
  const result = line.remake ? "remake" : line.win ? "win" : "loss";
  const minutes = Math.max(1, game.duration / 60);

  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={onSelect ? selected : undefined}
      className={cx(
        "group grid w-full min-w-0 items-center gap-x-4 rounded-10 py-2 pr-3 pl-0 text-left transition-colors duration-150 hairline",
        dense
          ? "grid-cols-[4px_auto_minmax(0,1fr)_auto_auto]"
          : "grid-cols-[4px_auto_minmax(0,1fr)_auto_auto_auto]",
        result === "win" ? "bg-win-soft" : result === "loss" ? "bg-loss-soft" : "bg-surface",
        selected ? "border-accent!" : "hover:border-border-strong",
      )}
    >
      <span
        aria-hidden
        className={cx(
          "h-full min-h-10 w-1 rounded-r-[3px]",
          result === "win" ? "bg-win" : result === "loss" ? "bg-loss" : "bg-border-strong",
        )}
      />

      <span className="relative">
        <ChampionIcon id={line.championId} size={dense ? 34 : 40} />
        {line.award && <AwardMark award={line.award} />}
        <span className="mono absolute -right-1.5 -bottom-1 rounded-4 bg-raised px-1 text-[10px] leading-3.5 text-fg-muted hairline">
          {line.championLevel}
        </span>
      </span>

      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="flex items-center gap-2">
          <span
            className={cx(
              "text-[13px] font-semibold",
              result === "win" ? "text-win" : result === "loss" ? "text-loss" : "text-fg-muted",
            )}
          >
            {result === "remake"
              ? t("common.remake")
              : result === "win"
                ? t("history.victory")
                : t("history.defeat")}
          </span>
          <span className="truncate text-[12.5px] text-fg">
            {queueName(game.queueId, game.gameMode, catalog, t("common.unknownQueue"))}
          </span>
          {line.placement !== null && (
            <span className="text-[12px] text-fg-muted">
              {t("history.placement", { n: line.placement })}
            </span>
          )}
        </span>
        <span className="flex min-w-0 items-center gap-2">
          <span className="mono shrink-0 text-[11px] text-fg-subtle">
            {relativeTime(game.startedAt, now, language)} · {gameLength(game.duration, language)}
          </span>
          <FeatChips feats={line.feats} limit={dense ? 2 : 3} />
        </span>
      </span>

      <span className="flex flex-col items-end gap-0.5">
        <span className="mono text-[13px] text-fg">
          {line.kills} <span className="text-fg-subtle">/</span>{" "}
          <span className="text-loss">{line.deaths}</span> <span className="text-fg-subtle">/</span>{" "}
          {line.assists}
        </span>
        <span className="text-[11px] text-fg-subtle">
          KDA <KdaValue kills={line.kills} deaths={line.deaths} assists={line.assists} />
        </span>
      </span>

      <span className="mono hidden flex-col items-end gap-0.5 text-[11px] text-fg-subtle @[760px]:flex">
        <span>
          {t("history.cs")} <span className="text-fg">{line.cs}</span> (
          {(line.cs / minutes).toFixed(1)})
        </span>
        <span>
          {t("history.damage")} <span className="text-fg">{compact(line.damage)}</span>
        </span>
      </span>

      {!dense && (
        <span className="flex items-center gap-2">
          {line.augments.length > 0 ? (
            // Augment modes (Hextech ARAM, Arena) have no runes: their augments take that place.
            <span className="flex items-center gap-1">
              <span className="flex flex-col gap-0.5">
                <AssetIcon kind="spells" id={line.spells[0]} size={18} />
                <AssetIcon kind="spells" id={line.spells[1]} size={18} />
              </span>
              <span
                className={cx(
                  "grid gap-0.5",
                  line.augments.length > 4 ? "grid-cols-3" : "grid-cols-2",
                )}
              >
                {line.augments.map((id) => (
                  <AugmentIcon key={id} id={id} size={18} />
                ))}
              </span>
            </span>
          ) : (
            <span className="grid grid-cols-2 gap-0.5">
              <AssetIcon kind="spells" id={line.spells[0]} size={18} />
              <AssetIcon kind="perks" id={line.keystone} size={18} />
              <AssetIcon kind="spells" id={line.spells[1]} size={18} />
              <AssetIcon kind="perks" id={line.subStyle} size={18} />
            </span>
          )}
          <span className="hidden grid-cols-4 gap-0.5 @[640px]:grid">
            {line.items.map((item, slot) => (
              <AssetIcon key={slot} kind="items" id={item} size={22} />
            ))}
          </span>
        </span>
      )}
    </button>
  );
}
