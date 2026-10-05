import { describe, expect, it } from "vitest";

import type { Rank } from "./bindings";
import {
  compact,
  duration,
  formatKda,
  gameLength,
  kda,
  percent,
  rankLabel,
  relativeTime,
  riotId,
  streakLabel,
  tierTone,
  winRate,
} from "./format";

const rank = (tier: Rank["tier"], division: string | null): Rank => ({
  tier,
  division,
  lp: 56,
  wins: 10,
  losses: 8,
});

describe("format", () => {
  it("joins a Riot ID and drops an empty tag", () => {
    expect(riotId({ gameName: "失了你", tagLine: "65740" })).toBe("失了你#65740");
    expect(riotId({ gameName: "Solo", tagLine: "" })).toBe("Solo");
    expect(riotId(null)).toBe("");
  });

  it("counts a deathless game as one death", () => {
    expect(kda(10, 0, 5)).toBe(15);
    expect(kda(2, 4, 6)).toBe(2);
    expect(formatKda(2.345)).toBe("2.3");
    expect(formatKda(15)).toBe("15");
  });

  it("has no win rate without games", () => {
    expect(winRate(0, 0)).toBeNull();
    expect(percent(winRate(3, 4))).toBe("75%");
    expect(percent(null)).toBe("—");
  });

  it("names ranks with and without divisions", () => {
    expect(rankLabel(rank("DIAMOND", "II"), "zh-CN")).toBe("璀璨钻石 II");
    expect(rankLabel(rank("DIAMOND", "II"), "zh-CN", true)).toBe("钻石 II");
    expect(rankLabel(rank("MASTER", null), "en")).toBe("Master");
  });

  it("calls two results in a row a streak", () => {
    expect(streakLabel(1, "zh-CN")).toBeNull();
    expect(streakLabel(3, "zh-CN")).toBe("3 连胜");
    expect(streakLabel(-2, "zh-CN")).toBe("2 连败");
    expect(streakLabel(-4, "en")).toBe("4L streak");
  });

  it("formats durations, ages and big numbers", () => {
    expect(duration(1925)).toBe("32:05");
    expect(duration(3760)).toBe("1:02:40");
    const now = Date.UTC(2026, 9, 5, 12);
    expect(relativeTime(now - 30_000, now, "zh-CN")).toBe("刚刚");
    expect(relativeTime(now - 5 * 60_000, now, "zh-CN")).toBe("5 分钟前");
    expect(relativeTime(now - 3 * 3_600_000, now, "en")).toBe("3h ago");
    const old = new Date(2026, 8, 7, 21, 8).getTime();
    expect(relativeTime(old, old + 30 * 86_400_000, "zh-CN")).toBe("9-07 21:08");
    expect(gameLength(1433, "zh-CN")).toBe("23分53秒");
    expect(gameLength(131, "en")).toBe("2m 11s");
    expect(compact(950)).toBe("950");
    expect(compact(12_345)).toBe("12k");
    expect(compact(5_432)).toBe("5.4k");
  });
});

describe("tierTone", () => {
  it("spreads any tier count over best, good, middle, weak and worst", () => {
    expect([0, 1, 2, 3, 4].map((tier) => tierTone(tier, 5))).toEqual([
      "best",
      "good",
      "middle",
      "weak",
      "worst",
    ]);
    expect([0, 1, 2].map((tier) => tierTone(tier, 3))).toEqual(["best", "middle", "worst"]);
    expect([0, 1, 2, 3].map((tier) => tierTone(tier, 4))).toEqual([
      "best",
      "good",
      "weak",
      "worst",
    ]);
    expect(tierTone(0, 1)).toBe("middle");
  });
});
