// The in-game callout as the core types it (crates/core/src/callout.rs): whose lines one press of
// the shortcut types into the game's chat, and how many at most.
import type { GameTeams } from "@winer/shared";

/** The most lines one press types into the game (`callout::GAME_LINE_LIMIT`): each holds the
 *  player's keyboard for about a second, so a team's first line and five players at most. */
export const GAME_LINE_LIMIT = 6;

/** What one press types under `teams` (`callout::typed`): the enemy lines, the team's, or both,
 *  the enemy's first, cut at the limit. */
export function typedLines(
  enemies: readonly string[],
  allies: readonly string[],
  teams: GameTeams,
): string[] {
  const chosen =
    teams === "enemies" ? enemies : teams === "allies" ? allies : [...enemies, ...allies];
  return chosen.slice(0, GAME_LINE_LIMIT);
}
