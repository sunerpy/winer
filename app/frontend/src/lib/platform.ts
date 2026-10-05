// Platform from the user agent: synchronous, and the only source vitest and a browser preview have.
// Mobile agents are screened first (Android's says Linux, iOS's says Mac OS X), and nothing
// WebKit-shaped is read as macOS before Windows and Linux are ruled out.
export type Platform = "macos" | "windows" | "linux" | "unknown";

export function detectPlatform(userAgent: string): Platform {
  if (/iPhone|iPad|iPod|Android/i.test(userAgent)) return "unknown";
  if (/Mac OS X|Macintosh/i.test(userAgent)) return "macos";
  if (/Windows/i.test(userAgent)) return "windows";
  if (/Linux|X11|CrOS|BSD/i.test(userAgent)) return "linux";
  return "unknown";
}

export function currentPlatform(): Platform {
  return typeof navigator === "undefined" ? "unknown" : detectPlatform(navigator.userAgent);
}
