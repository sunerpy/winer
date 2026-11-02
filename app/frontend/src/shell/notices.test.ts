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
      "needsAdmin",
      "noLines",
      "tooSoon",
    ];
    for (const reason of reasons) {
      const text = zh({ kind: "calloutSkipped", reason });
      expect(text, reason).not.toContain("callout.skip");
      expect(en({ kind: "calloutSkipped", reason }), reason).not.toContain("callout.skip");
      expect(isFailure({ kind: "calloutSkipped", reason }), "drawn as a failure").toBe(true);
    }
    expect(isFailure({ kind: "typedInGame", lines: 2 })).toBe(false);
    expect(zh({ kind: "calloutSkipped", reason: "needsAdmin" })).toBe(
      "喊话没有发送：游戏以管理员身份运行而 winer 不是，模拟的按键会被系统丢弃；以管理员身份重启 winer 后再试",
    );
  });

  it("says where a comeback went, where it stopped with how much was sent, and why not", () => {
    expect(zh({ kind: "comebackSent", place: "champSelect", messages: 1 })).toBe(
      "已把反击发到选人聊天",
    );
    expect(zh({ kind: "comebackSent", place: "team", messages: 1 })).toBe("已把反击发到队伍");
    expect(zh({ kind: "comebackSent", place: "all", messages: 2 })).toBe(
      "已把反击发到全体，分成 2 条消息",
    );
    expect(en({ kind: "comebackSent", place: "all", messages: 1 })).toBe(
      "Sent a comeback to everyone",
    );
    expect(zh({ kind: "comebackStopped", place: "team", sent: 1, reason: "notInFront" })).toBe(
      "反击输入到一半停下了：游戏窗口不在前台（已发到队伍 1 条）",
    );
    expect(en({ kind: "comebackStopped", place: "all", sent: 2, reason: "keysHeld" })).toBe(
      "The comeback stopped part of the way: a key stayed held down (2 sent to everyone)",
    );
    expect(zh({ kind: "comebackSkipped", place: "all", reason: "tooSoon" })).toBe(
      "反击没有发到全体：离上一句还不到 3 秒",
    );
    expect(zh({ kind: "comebackSkipped", place: "champSelect", reason: "noLines" })).toBe(
      "反击没有发到选人聊天：反击页还没有可以发的语句",
    );
    expect(en({ kind: "comebackSkipped", place: "team", reason: "needsAdmin" })).toBe(
      "Comeback not sent to the team: the game runs as administrator and winer does not, so the system would drop its keys; restart winer as administrator and try again",
    );
    expect(isFailure({ kind: "comebackSent", place: "team", messages: 1 })).toBe(false);
    expect(isFailure({ kind: "comebackStopped", place: "team", sent: 1, reason: "blocked" })).toBe(
      true,
    );
    expect(isFailure({ kind: "comebackSkipped", place: "all", reason: "inGameOff" })).toBe(true);
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
