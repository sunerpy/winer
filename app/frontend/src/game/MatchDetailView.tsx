// The scoreboard of one game: both teams, every player's line with winer's score, grade, MVP and
// SVP, damage share and kill participation, the game's best values picked out and, while titles
// are on, the roast title each line earned.
import {
  GRADE_LETTERS,
  compact,
  duration,
  percent,
  riotId,
  type Award,
  type MatchDetail,
  type PlayerLine,
  type TeamDetail,
} from "@winer/shared";
import type { ReactNode } from "react";

import { cx } from "../lib/cx";
import { useLanguage, useT } from "../lib/i18n";
import { GAME_TITLE, type GameTitle, TIER_NAMES, gameTitle } from "../lib/tiers";
import { FeatMarks } from "./Feats";
import { AssetIcon, AugmentIcon, ChampionIcon } from "./icons";
import { KdaValue, TitleChip } from "./stats";

// Player · KDA · score · damage · taken · KP · gold · CS · items. Taken and gold give way first
// when the scoreboard is narrow.
const COLUMNS =
  "grid-cols-[minmax(0,1fr)_76px_44px_116px_40px_36px_148px] @[860px]:grid-cols-[minmax(0,1fr)_80px_48px_128px_52px_44px_52px_40px_148px]";
const WIDE_ONLY = "hidden @[860px]:block";

function AwardBadge({ award }: { award: Award }) {
  return (
    <span
      className={cx(
        "shrink-0 rounded-4 px-1 text-[10px] leading-4 font-bold tracking-wide",
        award === "mvp" ? "bg-accent text-accent-fg" : "bg-fg/15 text-fg",
      )}
    >
      {award === "mvp" ? "MVP" : "SVP"}
    </span>
  );
}

/** The game's best value of each stat the scoreboard picks out, where one stands out. */
export interface Standouts {
  damage?: number;
  taken?: number;
  gold?: number;
  /** The fewest. */
  deaths?: number;
  score?: number;
}

/** Each stat's best value across the whole game. A stat everyone ties on, a best of nothing (no
 *  damage, no gold) and a remake pick out nobody, and one player alone has nobody to beat. */
export function standouts(detail: MatchDetail): Standouts {
  const lines = detail.teams.flatMap((team) => team.players);
  if (lines.length < 2 || lines.some((line) => line.remake)) return {};
  const best = (values: number[], fewest = false) => {
    const value = fewest ? Math.min(...values) : Math.max(...values);
    const tied = values.every((other) => other === value);
    return tied || (!fewest && value <= 0) ? undefined : value;
  };
  const scores = lines.flatMap((line) => (line.score === null ? [] : [line.score]));
  return {
    damage: best(lines.map((line) => line.damage)),
    taken: best(lines.map((line) => line.damageTaken)),
    gold: best(lines.map((line) => line.gold)),
    deaths: best(
      lines.map((line) => line.deaths),
      true,
    ),
    score: scores.length > 1 ? best(scores) : undefined,
  };
}

/** A value that is the game's best: an amber pill, and what it is best at for the tooltip and for
 *  screen readers. */
function Best({ label, children }: { label: string; children: ReactNode }) {
  return (
    // `relative` keeps the hidden label inside the pill; loose, it would widen the document.
    <span title={label} className="relative rounded-4 bg-best-soft px-1 font-semibold text-best">
      {children}
      <span className="sr-only">{label}</span>
    </span>
  );
}

/** The line's grade on the single-game bands, under its score. */
function Grade({ grade }: { grade: number }) {
  const t = useT();
  const language = useLanguage();
  const letter = GRADE_LETTERS[grade] ?? "F";
  const label = t("rating.grade", {
    grade: letter,
    name: TIER_NAMES.grades[language][grade] ?? "",
  });
  return (
    <span title={label} className="relative block text-[10.5px] font-semibold text-fg-subtle">
      <span aria-hidden>{letter}</span>
      <span className="sr-only">{label}</span>
    </span>
  );
}

function Score({ line, best }: { line: PlayerLine; best: boolean }) {
  const t = useT();
  if (line.score === null) return <span className="text-fg-subtle">—</span>;
  const tone =
    line.award === "mvp"
      ? "text-accent-text"
      : line.score >= 7.5
        ? "text-fg"
        : line.score < 4.5
          ? "text-fg-subtle"
          : "text-fg-muted";
  return (
    <>
      {best ? (
        <span className="mono text-[13px]">
          <Best label={t("history.best.score")}>{line.score.toFixed(1)}</Best>
        </span>
      ) : (
        <span className={cx("mono text-[13px] font-semibold", tone)}>{line.score.toFixed(1)}</span>
      )}
      {line.grade !== null && <Grade grade={line.grade} />}
    </>
  );
}

function HeaderRow() {
  const t = useT();
  const cell = "text-[10.5px] text-fg-subtle";
  return (
    <div className={cx("grid items-center gap-x-2 px-1.5 pb-1", COLUMNS)} aria-hidden>
      <span className={cell}>{t("history.player")}</span>
      <span className={cx(cell, "text-right")}>KDA</span>
      <span className={cx(cell, "text-right")} title={t("history.scoreHint")}>
        {t("history.score")}
      </span>
      <span className={cx(cell, "pl-3")}>{t("history.damage")}</span>
      <span className={cx(cell, WIDE_ONLY, "text-right")}>{t("history.taken")}</span>
      <span className={cx(cell, "text-right")}>{t("history.kp")}</span>
      <span className={cx(cell, WIDE_ONLY, "text-right")}>{t("history.gold")}</span>
      <span className={cx(cell, "text-right")}>{t("history.cs")}</span>
      <span className={cx(cell, "pr-1 text-right")}>{t("history.items")}</span>
    </div>
  );
}

function PlayerRow({
  player,
  maxDamage,
  bests,
  highlight,
  title,
  onPlayer,
}: {
  player: PlayerLine;
  maxDamage: number;
  bests: Standouts;
  highlight: boolean;
  title: GameTitle | null;
  onPlayer?: (puuid: string) => void;
}) {
  const t = useT();
  const name = riotId(player.name) || t("common.hidden");
  /** `value`, picked out when it is the game's best `stat`. */
  const stat = (key: keyof Standouts, value: number, shown: ReactNode = compact(value)) =>
    bests[key] === value ? <Best label={t(`history.best.${key}`)}>{shown}</Best> : shown;
  return (
    <li
      className={cx(
        "grid items-center gap-x-2 rounded-6 px-1.5 py-1 text-[12px]",
        COLUMNS,
        highlight ? "bg-accent-soft" : "bg-inset/60",
      )}
    >
      <span className="flex min-w-0 items-center gap-2">
        <ChampionIcon id={player.championId} size={26} />
        <span className="flex shrink-0 flex-col gap-px">
          <AssetIcon kind="spells" id={player.spells[0]} size={12} />
          <AssetIcon kind="spells" id={player.spells[1]} size={12} />
        </span>
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="flex min-w-0 items-center gap-2">
            {player.puuid && onPlayer ? (
              <button
                type="button"
                onClick={() => onPlayer(player.puuid)}
                className="min-w-0 truncate text-left text-fg hover:text-accent-text hover:underline"
              >
                {name}
              </button>
            ) : (
              <span className="min-w-0 truncate text-fg">{name}</span>
            )}
            {player.award && <AwardBadge award={player.award} />}
          </span>
          {(player.augments.length > 0 || player.feats.length > 0 || title) && (
            <span className="flex min-w-0 items-center gap-1">
              {player.augments.length > 0 && (
                <span className="flex shrink-0 gap-0.5">
                  {player.augments.map((id) => (
                    <AugmentIcon key={id} id={id} size={15} />
                  ))}
                </span>
              )}
              <FeatMarks feats={player.feats} />
              {title && (
                <TitleChip small name={t(GAME_TITLE[title].name)} why={t(GAME_TITLE[title].why)} />
              )}
            </span>
          )}
        </span>
      </span>
      <span className="mono text-right">
        {player.kills}/
        {bests.deaths === player.deaths ? (
          stat("deaths", player.deaths, player.deaths)
        ) : (
          <span className="text-loss">{player.deaths}</span>
        )}
        /{player.assists}
        <span className="block text-[10.5px] text-fg-subtle">
          <KdaValue kills={player.kills} deaths={player.deaths} assists={player.assists} />
        </span>
      </span>
      <span className="text-right">
        <Score line={player} best={bests.score !== undefined && bests.score === player.score} />
      </span>
      <span className="pl-3">
        <span className="mono flex items-baseline justify-between gap-1 text-[11px]">
          <span className="text-fg">{stat("damage", player.damage)}</span>
          {player.damageShare !== null && (
            <span className="text-fg-subtle">{percent(player.damageShare)}</span>
          )}
        </span>
        <span className="mt-0.5 block h-1 rounded-pill bg-inset2">
          <span
            className="block h-full rounded-pill bg-accent"
            style={{ width: `${maxDamage > 0 ? (player.damage / maxDamage) * 100 : 0}%` }}
          />
        </span>
      </span>
      <span className={cx("mono text-right text-fg-muted", WIDE_ONLY)}>
        {stat("taken", player.damageTaken)}
      </span>
      <span className="mono text-right text-fg-muted">{percent(player.killParticipation)}</span>
      <span className={cx("mono text-right text-fg-muted", WIDE_ONLY)}>
        {stat("gold", player.gold)}
      </span>
      <span className="mono text-right text-fg-muted">{player.cs}</span>
      <span className="flex justify-end gap-0.5 pr-0.5">
        {player.items.map((item, slot) => (
          <AssetIcon key={slot} kind="items" id={item} size={19} />
        ))}
      </span>
    </li>
  );
}

function TeamTable({
  team,
  mode,
  maxDamage,
  bests,
  objectives,
  titles,
  highlight,
  onPlayer,
}: {
  team: TeamDetail;
  /** The game's mode, which a title's counts are read against (`gameTitle`). */
  mode: string;
  maxDamage: number;
  bests: Standouts;
  /** Whether to count dragons and barons. */
  objectives: boolean;
  /** Whether to name the title each line earned. */
  titles: boolean;
  highlight?: string;
  onPlayer?: (puuid: string) => void;
}) {
  const t = useT();
  return (
    <section className="min-w-0">
      <header className="mb-1.5 flex items-center gap-3 px-1 text-[12px]">
        <span className={cx("font-semibold", team.win ? "text-win" : "text-loss")}>
          {team.win ? t("history.victory") : t("history.defeat")}
        </span>
        <span className="mono text-fg-subtle">
          {t("history.kills")} <span className="text-fg">{team.kills}</span> · {t("history.gold")}{" "}
          <span className="text-fg">{compact(team.gold)}</span> · {t("history.towers")}{" "}
          <span className="text-fg">{team.towers}</span>
          {objectives && (
            <>
              {" "}
              · {t("history.dragons")} <span className="text-fg">{team.dragons}</span> ·{" "}
              {t("history.barons")} <span className="text-fg">{team.barons}</span>
            </>
          )}
        </span>
        {team.bans.length > 0 && (
          <span className="ml-auto flex items-center gap-1">
            {team.bans.map((id) => (
              <ChampionIcon key={id} id={id} size={18} className="grayscale" />
            ))}
          </span>
        )}
      </header>
      <HeaderRow />
      <ol className="flex flex-col gap-0.5">
        {team.players.map((player) => (
          <PlayerRow
            key={player.puuid || player.championId}
            player={player}
            maxDamage={maxDamage}
            bests={bests}
            highlight={player.puuid === highlight}
            title={titles ? gameTitle(player, team.players, mode) : null}
            onPlayer={onPlayer}
          />
        ))}
      </ol>
    </section>
  );
}

export function MatchDetailView({
  detail,
  highlight,
  titles = false,
  onPlayer,
}: {
  detail: MatchDetail;
  highlight?: string;
  /** Name each line's roast title (`general.titles`). */
  titles?: boolean;
  onPlayer?: (puuid: string) => void;
}) {
  const t = useT();
  const maxDamage = Math.max(
    0,
    ...detail.teams.flatMap((team) => team.players.map((player) => player.damage)),
  );
  // Dragons and barons are Summoner's Rift's: where neither side took one, as on every ARAM map,
  // a pair of zeros would only be noise.
  const objectives = detail.teams.some((team) => team.dragons > 0 || team.barons > 0);
  const bests = standouts(detail);
  return (
    <div className="@container flex flex-col gap-4">
      <p className="mono text-[11px] text-fg-subtle">
        {t("history.duration")} {duration(detail.duration)} · {t("history.version")}{" "}
        {detail.gameVersion.split(".").slice(0, 2).join(".")} · #{detail.gameId}
      </p>
      {detail.teams.map((team) => (
        <TeamTable
          key={team.teamId}
          team={team}
          mode={detail.gameMode}
          maxDamage={maxDamage}
          bests={bests}
          objectives={objectives}
          titles={titles}
          highlight={highlight}
          onPlayer={onPlayer}
        />
      ))}
    </div>
  );
}
