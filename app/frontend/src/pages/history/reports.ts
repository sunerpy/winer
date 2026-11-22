import { zipSync } from "fflate";
import { toBlob } from "html-to-image";
import { flushSync } from "react-dom";
import type { MatchSummary } from "@winer/shared";

export function reportName(game: MatchSummary, player: string, champion: string, result: string) {
  const at = new Date(game.startedAt);
  const day = [
    at.getFullYear(),
    String(at.getMonth() + 1).padStart(2, "0"),
    String(at.getDate()).padStart(2, "0"),
  ].join("-");
  const safe = (text: string) =>
    Array.from(text.replace(/[<>:"/\\|?*\p{Cc}]/gu, "_"))
      .slice(0, 48)
      .join("")
      .replace(/[. ]+$/u, "");
  const prefix = `${day}_${safe(player)}_${safe(champion)}_${safe(result)}`;
  const id = String(game.gameId);
  return `${Array.from(prefix)
    .slice(0, 79 - id.length)
    .join("")}_${id}.png`;
}

/** A deadline also covers a stalled asset or webfont, so one game cannot stall the batch. */
export async function withDeadline<T>(work: Promise<T>, milliseconds = 30000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      work,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error("Battle report timed out")), milliseconds);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export async function captureReport(node: HTMLElement): Promise<Uint8Array> {
  // The existing icons are lazy-loaded. Offscreen reports must explicitly start their requests.
  for (const img of node.querySelectorAll("img")) img.loading = "eager";
  await document.fonts.ready;
  await Promise.all(
    Array.from(node.querySelectorAll("img"), (img) =>
      img.decode().catch(() => {
        // Use GameImage's existing fallback, including when decode rejects before onerror fires.
        flushSync(() => img.dispatchEvent(new Event("error")));
      }),
    ),
  );
  // Failed client assets have React fallbacks; let those state updates commit before cloning.
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  const blob = await toBlob(node, { pixelRatio: 2, preferredFontFormat: "woff2" });
  if (!blob) throw new Error("PNG generation failed");
  return new Uint8Array(await blob.arrayBuffer());
}

export async function collectReports<T>(
  games: readonly T[],
  render: (game: T, index: number) => Promise<{ name: string; png: Uint8Array }>,
  save?: (name: string, png: Uint8Array) => Promise<unknown>,
) {
  const files: Record<string, Uint8Array> = {};
  let failed = 0;
  let succeeded = 0;
  for (const [index, game] of games.entries()) {
    try {
      const { name, png } = await render(game, index);
      if (save) await save(name, png);
      else files[name] = png;
      succeeded += 1;
    } catch (error) {
      console.warn("Battle report failed", error);
      failed += 1;
    }
  }
  return { files, failed, succeeded };
}

// PNG already compresses its pixels. Store entries without recompressing them.
export function packReports(files: Record<string, Uint8Array>): Uint8Array {
  return zipSync(files, { level: 0 });
}
