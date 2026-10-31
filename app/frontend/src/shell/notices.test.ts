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

  it("says what the automatic claim did", () => {
    expect(zh({ kind: "rewardsClaimed", claimed: 4, left: 0 })).toBe("自动领取了 4 个奖励");
    expect(zh({ kind: "rewardsClaimed", claimed: 3, left: 1 })).toBe(
      "自动领取了 3 个奖励，还有 1 个待领取",
    );
    expect(en({ kind: "rewardsClaimed", claimed: 3, left: 1 })).toBe(
      "Claimed rewards automatically: 3; 1 still waiting",
    );
    expect(zh({ kind: "rewardsFailed", failed: 2 })).toBe("有 2 个奖励没能自动领取，到工具页看看");
    expect(isFailure({ kind: "rewardsFailed", failed: 2 })).toBe(true);
    expect(isFailure({ kind: "rewardsClaimed", claimed: 1, left: 0 })).toBe(false);
  });
});
