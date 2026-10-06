// What the profile panels of the Tools page compute: which skins a search finds, how a backup is
// described, and the ranks a disguise can claim.
import type { BackupChannel, ChampionInfo, Division, SkinChoice, Tier } from "@winer/shared";

/** Skins on one page of the picker: four rows of six on a wide window. */
export const SKIN_PAGE_SIZE = 24;

/** Whether `query` names the skin or its champion: the skin's own name, the champion's title and
 *  short name as the client shows them, or its English key (`Ahri`), so either language finds it. */
export function matchesSkin(
  skin: SkinChoice,
  champion: ChampionInfo | undefined,
  query: string,
): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const names = [skin.name, champion?.name, champion?.shortName, champion?.alias];
  return names.some((name) => name?.toLowerCase().includes(needle));
}

/** The skins the picker shows for a search, a champion and the owned switch, in the client's order. */
export function filterSkins(
  skins: readonly SkinChoice[],
  champions: ReadonlyMap<number, ChampionInfo> | undefined,
  { query, champion, ownedOnly }: { query: string; champion: number | null; ownedOnly: boolean },
): SkinChoice[] {
  return skins.filter(
    (skin) =>
      (!ownedOnly || skin.owned) &&
      (champion === null || skin.championId === champion) &&
      matchesSkin(skin, champions?.get(skin.championId), query),
  );
}

/** Every tier, lowest first, as the disguise offers them. */
export const TIERS: readonly Tier[] = [
  "IRON",
  "BRONZE",
  "SILVER",
  "GOLD",
  "PLATINUM",
  "EMERALD",
  "DIAMOND",
  "MASTER",
  "GRANDMASTER",
  "CHALLENGER",
];

export const DIVISIONS: readonly Division[] = ["I", "II", "III", "IV"];

/** Master and up have no divisions. */
export function hasDivisions(tier: Tier): boolean {
  return TIERS.indexOf(tier) < TIERS.indexOf("MASTER");
}

/** What a restore can put back: one half of the game's settings or both. */
export type RestoreChoice = BackupChannel | "all";

export function restoreChannels(choice: RestoreChoice): BackupChannel[] {
  return choice === "all" ? ["general", "hotkeys"] : [choice];
}

/** `8.2 KB`: a backup is a few kilobytes. */
export function backupSize(bytes: number): string {
  return `${(Math.max(0, bytes) / 1024).toFixed(1)} KB`;
}

/** `2026-10-06 14:05`, local time: a backup is found again by when it was made. */
export function backupTime(at: number): string {
  const date = new Date(at);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
