// The History page's sums and files: each champion's games over what the list has loaded, and the
// games as CSV or JSON for the user's Downloads (`save_export`). Remakes count toward nothing.
import type { MatchSummary } from "@winer/shared";

export interface ChampionRow {
  championId: number;
  games: number;
  wins: number;
  kills: number;
  deaths: number;
  assists: number;
  /** The average game score, 0–10; `null` without a scored game. */
  score: number | null;
  mvp: number;
  svp: number;
  /** When the latest of these games started, epoch milliseconds. */
  last: number;
}

/** Each champion's games among `games`, remakes left out: the most played first, then the most
 *  recently played, then by id. */
export function championRows(games: readonly MatchSummary[]): ChampionRow[] {
  const rows = new Map<number, ChampionRow & { scored: number; total: number }>();
  for (const game of games) {
    const line = game.line;
    if (line.remake) continue;
    const row = rows.get(line.championId) ?? {
      championId: line.championId,
      games: 0,
      wins: 0,
      kills: 0,
      deaths: 0,
      assists: 0,
      score: null,
      mvp: 0,
      svp: 0,
      last: 0,
      scored: 0,
      total: 0,
    };
    row.games += 1;
    if (line.win) row.wins += 1;
    row.kills += line.kills;
    row.deaths += line.deaths;
    row.assists += line.assists;
    if (line.score !== null) {
      row.scored += 1;
      row.total += line.score;
    }
    if (line.award === "mvp") row.mvp += 1;
    if (line.award === "svp") row.svp += 1;
    row.last = Math.max(row.last, game.startedAt);
    rows.set(line.championId, row);
  }
  return [...rows.values()]
    .map(({ scored, total, ...row }) => ({ ...row, score: scored > 0 ? total / scored : null }))
    .sort((a, b) => b.games - a.games || b.last - a.last || a.championId - b.championId);
}

/** One CSV field: quoted, with its quotes doubled, when it holds a comma, a quote or a break. */
export function csvField(value: string | number | null): string {
  const text = value === null ? "" : String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

/** What one exported game says, in the order of the CSV's columns. */
interface ExportedGame {
  startedAt: string;
  queue: string;
  champion: string;
  result: string;
  kills: number;
  deaths: number;
  assists: number;
  score: number | null;
  cs: number;
  minutes: number;
  gameId: number;
}

export interface ExportWords {
  /** The CSV's header, one name for each of `ExportedGame`'s fields in order. */
  header: readonly string[];
  champion: (id: number) => string;
  queue: (game: MatchSummary) => string;
  result: (game: MatchSummary) => string;
}

function exported(game: MatchSummary, words: ExportWords): ExportedGame {
  const line = game.line;
  return {
    startedAt: new Date(game.startedAt).toISOString(),
    queue: words.queue(game),
    champion: words.champion(line.championId),
    result: words.result(game),
    kills: line.kills,
    deaths: line.deaths,
    assists: line.assists,
    score: line.score === null ? null : Math.round(line.score * 10) / 10,
    cs: line.cs,
    minutes: Math.round(game.duration / 6) / 10,
    gameId: game.gameId,
  };
}

/** The games as CSV, a header line first; the player's own line only. */
export function gamesCsv(games: readonly MatchSummary[], words: ExportWords): string {
  const rows = games.map((game) => Object.values(exported(game, words)).map(csvField).join(","));
  return `${[words.header.map(csvField).join(","), ...rows].join("\r\n")}\r\n`;
}

/** The games as JSON, the same fields as the CSV's. */
export function gamesJson(games: readonly MatchSummary[], words: ExportWords): string {
  return `${JSON.stringify(
    games.map((game) => exported(game, words)),
    null,
    2,
  )}\n`;
}

/** A file name's stem for `name`'s games on `at`'s day: what Windows refuses in a name becomes
 *  `_`, and the whole stays short (`save_export` checks it again). */
export function exportStem(prefix: string, name: string, at: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  const day = `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
  const safe = Array.from(`${prefix} ${name}`.replace(/[<>:"/\\|?*\p{Cc}]/gu, "_"))
    .slice(0, 60)
    .join("")
    .trim()
    .replace(/[. ]+$/u, "");
  return `${safe} ${day}`;
}
