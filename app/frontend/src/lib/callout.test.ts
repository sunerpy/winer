import { describe, expect, it } from "vitest";

import { GAME_LINE_LIMIT, typedLines } from "./callout";

describe("the in-game callout", () => {
  const enemies = ["敌方", "小心", "对面"];
  const allies = ["我方", "一", "二", "三", "四", "五"];

  it("types the chosen team's lines, the enemy's first, as the core does", () => {
    expect(typedLines(enemies, allies, "enemies")).toEqual(enemies);
    expect(typedLines(enemies, allies, "allies"), "a team's first line and five fit").toEqual(
      allies,
    );
    expect(typedLines(enemies, allies, "both")).toEqual([
      "敌方",
      "小心",
      "对面",
      "我方",
      "一",
      "二",
    ]);
    expect(GAME_LINE_LIMIT).toBe(6);
    expect(typedLines([], allies.slice(0, 2), "both")).toEqual(["我方", "一"]);
  });
});
