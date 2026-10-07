import type { Check, DiagnosticsReport } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { translate } from "../../lib/i18n";
import { checkLine, detailBlock, failureText, reportText } from "./diagnostics";

const t = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate("zh-CN", key, params);

const check = (change: Partial<Check>): Check => ({
  id: "client",
  status: "ok",
  reason: "fine",
  detail: null,
  failure: null,
  tookMs: 0,
  ...change,
});

describe("diagnostics", () => {
  it("words each check from its reason, its version and how a request failed", () => {
    expect(checkLine(check({ detail: "16.20.1" }), t)).toBe("正常 · 16.20.1");
    expect(
      checkLine(
        check({
          id: "sgp",
          status: "fail",
          reason: "unreachable",
          failure: { kind: "status", code: 401 },
        }),
        t,
      ),
    ).toBe("请求失败 · HTTP 401");
    expect(
      checkLine(
        check({
          id: "plugin",
          status: "warn",
          reason: "foreignLoader",
          detail: "Pengu Loader.exe",
        }),
        t,
      ),
    ).toBe("Pengu Loader.exe 接管了客户端启动，winer 没有链接自带的 loader");
    expect(failureText({ kind: "timeout" }, t)).toBe("超时");
    expect(detailBlock(check({ reason: "foreignLoader", detail: "x" }))).toBeNull();
  });

  it("copies as one line a check, the missing routes indented under theirs", () => {
    const report: DiagnosticsReport = {
      at: new Date(2026, 9, 8, 9, 5).getTime(),
      clientVersion: "16.20.1",
      checks: [
        check({ detail: "16.20.1", tookMs: 12 }),
        check({
          id: "routes",
          status: "fail",
          reason: "routesMissing",
          detail: "PATCH /lol-champ-select/v1/session/actions/{id}\nPOST /riotclient/ux-show",
        }),
        check({ id: "sourceOpgg", status: "skipped", reason: "sourceOff" }),
        check({ id: "updater", status: "warn", reason: "checkFailed" }),
      ],
    };
    expect(reportText(report, "0.0.8", t)).toBe(
      [
        "winer 0.0.8 · 诊断 · 2026-10-08 09:05",
        "客户端 16.20.1",
        "✓ 客户端 · 正常 · 16.20.1",
        "✗ 客户端接口 · 客户端缺少以下 winer 用到的接口",
        "    PATCH /lol-champ-select/v1/session/actions/{id}",
        "    POST /riotclient/ux-show",
        "– OP.GG · 设置里没有用到，未检查",
        "! 更新 · 最近一次检查更新失败",
      ].join("\n"),
    );
  });
});
