import type { ComebackSettings } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { DEFAULT_PHRASES, fill, fillPhrase, phrasesOf, seatLabel } from "./comeback";

describe("the Comebacks page's quick phrases", () => {
  const rule: ComebackSettings = {
    lines: [],
    hotkey: null,
    allHotkey: null,
    inGame: false,
    phrases: [],
    phraseHotkey: null,
    library: { enabled: false, level: "mild" },
  };

  it("offers the language's own phrase while the user has none, as the core numbers them", () => {
    expect(phrasesOf(rule, "zh-CN")).toEqual(["{seat}大佬，小弟想玩下{champion}，给您磕一个了"]);
    expect(phrasesOf(rule, "en")).toEqual(DEFAULT_PHRASES.en);
    expect(phrasesOf({ ...rule, phrases: ["换吗", "谢谢"] }, "zh-CN")).toEqual(["换吗", "谢谢"]);
  });

  it("names a seat as the callout does", () => {
    expect(seatLabel(5, "zh-CN")).toBe("5L");
    expect(seatLabel(2, "en")).toBe("P2");
  });

  it("fills a phrase for one teammate, a blank value taking its brackets along", () => {
    const [zh] = DEFAULT_PHRASES["zh-CN"];
    expect(fillPhrase(zh ?? "", "5L", "龙王", "峡谷清道夫")).toBe(
      "5L大佬，小弟想玩下龙王，给您磕一个了",
    );
    expect(fillPhrase("{seat}，换【{champion}】吗？", "2L", "", "")).toBe("2L，换吗？");
    expect(fillPhrase("{seat} [{name}] {champion} please", "P1", "Ahri", "")).toBe(
      "P1 Ahri please",
    );
    expect(fillPhrase("【】{seat}", "P3", "Ahri", "Bo"), "a pair the user wrote empty stays").toBe(
      "【】P3",
    );
    expect(
      fill("{seat} | {name}", [
        ["{seat}", "1L"],
        ["{name}", ""],
      ]),
      "no separator left",
    ).toBe("1L");
    expect(fillPhrase("$& {name}", "P1", "Ahri", "$1"), "values are taken as they are").toBe(
      "$& $1",
    );
  });
});
