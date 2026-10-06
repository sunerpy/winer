import type { CleanupReport } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { translate } from "../../lib/i18n";
import { cleanupLines, formatSize, sizeParts } from "./storage";

const zh = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate("zh-CN", key, params);
const en = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate("en", key, params);

const NONE = { files: 0, bytes: 0 };
const nothing: CleanupReport = {
  logs: NONE,
  updates: NONE,
  webviewCache: NONE,
  memory: { entries: 0, imageBytes: 0 },
};

describe("storage", () => {
  it("says a size in the unit that fits it, one decimal under ten", () => {
    expect(sizeParts(0)).toEqual(["0", "B"]);
    expect(sizeParts(512)).toEqual(["512", "B"]);
    expect(sizeParts(1024)).toEqual(["1", "KB"]);
    expect(formatSize(15_204)).toBe("15 KB");
    expect(formatSize(9_369_496)).toBe("8.9 MB");
    expect(formatSize(78_975_184)).toBe("75 MB");
    expect(formatSize(50 * 1024 * 1024)).toBe("50 MB");
    expect(formatSize(3 * 1024 ** 4)).toBe("3072 GB");
    expect(formatSize(-5)).toBe("0 B");
  });

  it("says what a cleanup removed now and what goes at the next start", () => {
    const report: CleanupReport = {
      logs: { files: 2, bytes: 314_572 },
      updates: NONE,
      webviewCache: { files: 58, bytes: 9_369_496 },
      memory: { entries: 523, imageBytes: 9_961_472 },
    };
    expect(cleanupLines(report, zh)).toEqual([
      "已清理旧日志 2 个（307 KB）、内存缓存 523 项。",
      "网页缓存 8.9 MB 将在下次启动 winer 时清除。",
    ]);
    expect(cleanupLines(report, en)).toEqual([
      "Removed 2 old log files (307 KB), 523 entries cached in memory.",
      "The WebView's cache, 8.9 MB, goes the next time winer starts.",
    ]);
    expect(cleanupLines(nothing, zh)).toEqual(["没有需要清理的内容。"]);
    expect(cleanupLines({ ...nothing, updates: { files: 1, bytes: 6_291_456 } }, zh)).toEqual([
      "已清理更新安装包 1 个（6 MB）。",
    ]);
  });
});
