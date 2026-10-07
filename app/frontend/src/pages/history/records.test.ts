import type { MatchSummary, PlayerLine } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { championRows, csvField, exportStem, gamesCsv, gamesJson } from "./records";

const game = (
  gameId: number,
  championId: number,
  line: Partial<PlayerLine>,
  startedAt = gameId * 1000,
): MatchSummary =>
  ({
    gameId,
    queueId: 420,
    gameMode: "CLASSIC",
    startedAt,
    duration: 1830,
    kind: "players",
    line: {
      championId,
      kills: 1,
      deaths: 1,
      assists: 1,
      cs: 150,
      win: false,
      remake: false,
      score: null,
      award: null,
      ...line,
    },
  }) as unknown as MatchSummary;

const words = {
  header: ["Started", "Mode", "Champion", "Result", "K", "D", "A", "Score", "CS", "Min", "Id"],
  champion: (id: number) => (id === 103 ? "阿狸, 九尾妖狐" : `#${id}`),
  queue: () => "排位",
  result: (summary: MatchSummary) => (summary.line.win ? "胜" : "负"),
};

describe("history records", () => {
  it("sums each champion's games without the remakes, the most played first", () => {
    const rows = championRows([
      game(1, 103, { win: true, kills: 10, score: 8, award: "mvp" }),
      game(2, 103, { win: false, deaths: 5, score: 6 }),
      game(3, 64, { win: true, award: "svp" }, 9_000),
      game(4, 64, { remake: true, win: true }),
      game(5, 86, { win: true }, 5_000),
    ]);
    expect(rows.map((row) => [row.championId, row.games, row.wins])).toEqual([
      [103, 2, 1],
      [64, 1, 1],
      [86, 1, 1],
    ]);
    expect(rows[0]).toMatchObject({ kills: 11, deaths: 6, score: 7, mvp: 1, svp: 0, last: 2000 });
    expect(rows[1]).toMatchObject({ score: null, svp: 1, last: 9000 });
  });

  it("writes CSV fields quoted only when they must be, and the same fields as JSON", () => {
    expect(csvField("plain")).toBe("plain");
    expect(csvField('say "hi", ok')).toBe('"say ""hi"", ok"');
    expect(csvField("two\nlines")).toBe('"two\nlines"');
    expect(csvField(null)).toBe("");
    expect(csvField(3.5)).toBe("3.5");

    const games = [game(7, 103, { win: true, score: 7.46 })];
    const csv = gamesCsv(games, words);
    const [header, row] = csv.split("\r\n");
    expect(header).toBe("Started,Mode,Champion,Result,K,D,A,Score,CS,Min,Id");
    expect(row).toBe(
      `${new Date(7000).toISOString()},排位,"阿狸, 九尾妖狐",胜,1,1,1,7.5,150,30.5,7`,
    );
    const [exported] = JSON.parse(gamesJson(games, words)) as Record<string, unknown>[];
    expect(exported).toMatchObject({
      champion: "阿狸, 九尾妖狐",
      result: "胜",
      score: 7.5,
      gameId: 7,
    });
    expect(exported).not.toHaveProperty("puuid");
  });

  it("names the file after the player and the day, without what Windows refuses", () => {
    const day = new Date(2026, 9, 8, 21, 30);
    expect(exportStem("winer 战绩", "Ann#1", day)).toBe("winer 战绩 Ann#1 2026-10-08");
    expect(exportStem("winer 战绩", 'a/b\\c:d*e?"f"<g>|h', day)).toBe(
      "winer 战绩 a_b_c_d_e__f__g__h 2026-10-08",
    );
    expect(exportStem("x", "name. . .", day)).toBe("x name 2026-10-08");
  });
});
