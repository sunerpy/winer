// What the History page has shown, kept so that going back to a player draws at once, where the
// page was left, and only then asks whether new games came in. The core keeps the requests behind
// it as well (crates/core/src/history.rs); this saves the round trip and the page's place.
// Everything belongs to the account signed in when it was shown: another account empties it.
import type { HistorySource, MatchDetail, MatchSummary, PlayerSummary } from "@winer/shared";
import type { DependencyList } from "react";

import { type AsyncState, useAsync } from "./useAsync";

/** A map that lets the least recently used entries go past `limit`. */
export class Lru<K, V> {
  readonly #entries = new Map<K, V>();

  constructor(readonly limit: number) {}

  get(key: K): V | undefined {
    const value = this.#entries.get(key);
    if (value !== undefined) {
      this.#entries.delete(key);
      this.#entries.set(key, value);
    }
    return value;
  }

  set(key: K, value: V): void {
    this.#entries.delete(key);
    this.#entries.set(key, value);
    for (const oldest of this.#entries.keys()) {
      if (this.#entries.size <= this.limit) break;
      this.#entries.delete(oldest);
    }
  }

  clear(): void {
    this.#entries.clear();
  }

  get size(): number {
    return this.#entries.size;
  }
}

/** One player's list as the History page left it. */
export interface HistoryList {
  /** Every game read so far, newest first. */
  games: MatchSummary[];
  /** Where the next request starts. */
  next: number;
  more: boolean;
  source: HistorySource | null;
  page: number;
  filter: string;
}

export class HistoryCache {
  #viewer: string | null = null;
  readonly #lists = new Lru<string, HistoryList>(12);
  readonly #details = new Lru<number, MatchDetail>(200);
  readonly #summaries = new Lru<string, PlayerSummary>(40);

  /** The account signed in now; another than before empties the cache. While none is (the client
   *  closed), what there is stays for the same account to come back to. */
  scope(viewer: string | null): void {
    if (viewer === null || viewer === this.#viewer) return;
    this.#viewer = viewer;
    this.#lists.clear();
    this.#details.clear();
    this.#summaries.clear();
  }

  get viewer(): string | null {
    return this.#viewer;
  }

  list(viewer: string, puuid: string): HistoryList | undefined {
    return viewer === this.#viewer ? this.#lists.get(puuid) : undefined;
  }

  /** Keeps `list`, unless it was read for an account that has signed out since. */
  putList(viewer: string, puuid: string, list: HistoryList): void {
    if (viewer === this.#viewer) this.#lists.set(puuid, list);
  }

  detail(viewer: string, gameId: number): MatchDetail | undefined {
    return viewer === this.#viewer ? this.#details.get(gameId) : undefined;
  }

  putDetail(viewer: string, detail: MatchDetail): void {
    if (viewer === this.#viewer) this.#details.set(detail.gameId, detail);
  }

  summary(viewer: string, puuid: string): PlayerSummary | undefined {
    return viewer === this.#viewer ? this.#summaries.get(puuid) : undefined;
  }

  putSummary(viewer: string, summary: PlayerSummary): void {
    if (viewer === this.#viewer) this.#summaries.set(summary.puuid, summary);
  }
}

/** `load`'s answer, drawn from `cached` at once where there is one. With `refresh` it is still
 *  asked for and replaces what was drawn; without, a cached value is final. Every answer goes to
 *  `keep`. */
export function useCached<T>(
  cached: T | undefined,
  load: () => Promise<T>,
  keep: (value: T) => void,
  { refresh, deps, enabled = true }: { refresh: boolean; deps: DependencyList; enabled?: boolean },
): AsyncState<T> {
  const state = useAsync(
    () =>
      load().then((value) => {
        keep(value);
        return value;
      }),
    deps,
    enabled && (refresh || cached === undefined),
  );
  return { ...state, data: state.data ?? cached };
}
