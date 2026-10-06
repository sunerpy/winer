// The demo client's storage (`demo.ts`, Settings › About): what a Windows install held after two
// days of use, as measured on one (`docs/platform-notes.md`), with an update installer left behind,
// and a cleanup that removes what the shell's would.
import type { CleanupReport, DiskUse, StorageReport } from "@winer/shared";

import type { Commands } from "./backend";

type StorageCommand = "get_storage" | "clear_caches";

const MB = 1024 * 1024;
const NONE: DiskUse = { files: 0, bytes: 0 };

/** The demo's answers to Settings › About's storage block. */
export function demoStorageHandlers(): {
  [K in StorageCommand]: (args: Commands[K]["args"]) => Commands[K]["result"];
} {
  let report: StorageReport = {
    logs: { files: 2, bytes: 15_204 },
    webview: { files: 361, bytes: 78_975_184 },
    webviewCache: { files: 63, bytes: 9_927_920 },
    webviewClearPending: false,
    backups: { files: 2, bytes: 14_279 },
    pengu: { files: 4, bytes: 508_204 },
    settings: { files: 2, bytes: 3_120 },
    updates: { files: 1, bytes: 6_291_456 },
    memory: { entries: 523, imageBytes: 12_582_912 },
    limits: {
      logDays: 7,
      logBytes: 50 * MB,
      logFileBytes: 10 * MB,
      webviewCacheBytes: 32 * MB,
      backups: 10,
      imageBytes: 32 * MB,
    },
  };
  return {
    get_storage: () => report,
    clear_caches: () => {
      // As the shell does: every log file but today's, the spent installers and the memory now;
      // the WebView's caches at the next start.
      const today = { files: 1, bytes: 14_341 };
      const cleaned: CleanupReport = {
        logs: {
          files: report.logs.files - today.files,
          bytes: report.logs.bytes - today.bytes,
        },
        updates: report.updates,
        webviewCache: report.webviewCache,
        memory: report.memory,
      };
      report = {
        ...report,
        logs: today,
        updates: NONE,
        webviewClearPending: report.webviewCache.files > 0,
        memory: { entries: 0, imageBytes: 0 },
      };
      return cleaned;
    },
  };
}
