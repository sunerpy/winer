import type { MatchSummary } from "@winer/shared";
import { unzipSync } from "fflate";
import { describe, expect, it } from "vitest";
import { collectReports, packReports, reportName, withDeadline } from "./reports";

describe("battle report export", () => {
  it("keeps Unicode names safe on Windows and preserves unique game IDs", () => {
    const game = { startedAt: new Date(2026, 10, 5).getTime(), gameId: 123 } as MatchSummary;
    expect(reportName(game, '玩家:/\\*?"<>|\n', "阿狸", "胜利")).toBe(
      "2026-11-05_玩家___________阿狸_胜利_123.png",
    );
    expect(reportName(game, "x".repeat(200), "阿狸", "胜利").length).toBeLessThan(100);
  });

  it("continues after a failed game and packs the successful PNG bytes with Unicode filenames", async () => {
    const png = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
    const visited: number[] = [];
    const result = await collectReports([1, 2, 3], async (id) => {
      visited.push(id);
      if (id === 2) throw new Error("detail unavailable");
      return { name: `玩家_${id}.png`, png };
    });
    expect(visited).toEqual([1, 2, 3]);
    expect(result).toMatchObject({ succeeded: 2, failed: 1 });
    const files = unzipSync(packReports(result.files));
    expect(Object.keys(files)).toEqual(["玩家_1.png", "玩家_3.png"]);
    expect(files["玩家_3.png"]).toEqual(png);
  });

  it("saves each PNG immediately, counts write failures and continues without keeping PNGs", async () => {
    const saved: string[] = [];
    const result = await collectReports(
      [1, 2, 3],
      async (id) => ({ name: id + ".png", png: new Uint8Array([id]) }),
      async (name, png) => {
        if (png[0] === 2) throw new Error("disk full");
        saved.push(name);
      },
    );
    expect(saved).toEqual(["1.png", "3.png"]);
    expect(result).toEqual({ files: {}, succeeded: 2, failed: 1 });
  });

  it("counts all failures and times out stalled work", async () => {
    const result = await collectReports([1], () => withDeadline(new Promise<never>(() => {}), 5));
    expect(result).toEqual({ files: {}, succeeded: 0, failed: 1 });
  });
});
