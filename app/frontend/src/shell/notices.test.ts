import type { CalloutSkip, NoticeKind } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { translate } from "../lib/i18n";
import { isFailure, noticeText } from "./notices";

describe("notices", () => {
  const zh = (kind: NoticeKind) =>
    noticeText(kind, (key, params) => translate("zh-CN", key, params), null);
  const en = (kind: NoticeKind) =>
    noticeText(kind, (key, params) => translate("en", key, params), null);

  it("says what the callout's shortcut did, and why it sent nothing", () => {
    expect(zh({ kind: "typedInGame", lines: 3 })).toBe("已在游戏聊天输入 3 条喊话");
    expect(zh({ kind: "calledOut", lines: 6 })).toBe("已发送 6 条战力喊话");
    expect(zh({ kind: "typingStopped", lines: 1, reason: "notInFront" })).toBe(
      "喊话输入到一半停下了：游戏窗口不在前台（已发送 1 条）",
    );
    expect(zh({ kind: "calloutSkipped", reason: "inGameOff" })).toBe(
      "喊话没有发送：游戏内发送未开启",
    );
    expect(en({ kind: "calloutSkipped", reason: "notInFront" })).toBe(
      "Callout not sent: the game's window is not in front",
    );
    const reasons: CalloutSkip[] = [
      "notNow",
      "nothingToSay",
      "inGameOff",
      "notInFront",
      "keysHeld",
      "blocked",
      "unsupported",
    ];
    for (const reason of reasons) {
      const text = zh({ kind: "calloutSkipped", reason });
      expect(text, reason).not.toContain("callout.skip");
      expect(isFailure({ kind: "calloutSkipped", reason }), "drawn as a failure").toBe(true);
    }
    expect(isFailure({ kind: "typedInGame", lines: 2 })).toBe(false);
  });
});
