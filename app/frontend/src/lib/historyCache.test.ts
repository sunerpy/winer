import type { Event, MatchDetail, MatchSummary, PlayerSummary, Update } from "@winer/shared";
import { describe, expect, it } from "vitest";

import type { Backend } from "./backend";
import { HistoryCache, type HistoryList, Lru } from "./historyCache";
import { AppStore, EMPTY_SNAPSHOT } from "./store";

const list = (games: number[]): HistoryList => ({
  games: games.map((gameId) => ({ gameId }) as MatchSummary),
  next: 50,
  more: true,
  source: "server",
  page: 2,
  filter: "ranked",
});

const summary = (puuid: string) => ({ puuid }) as PlayerSummary;
const detail = (gameId: number) => ({ gameId }) as MatchDetail;

describe("history cache", () => {
  it("lets the least recently used entries go", () => {
    const lru = new Lru<string, number>(2);
    lru.set("a", 1);
    lru.set("b", 2);
    expect(lru.get("a")).toBe(1);
    lru.set("c", 3);
    expect([lru.get("a"), lru.get("b"), lru.get("c")]).toEqual([1, undefined, 3]);
    lru.set("a", 4);
    expect(lru.size).toBe(2);
    expect(lru.get("a")).toBe(4);
  });

  it("keeps what one account was shown for that account only", () => {
    const cache = new HistoryCache();
    cache.scope("me");
    cache.putList("me", "p", list([3, 2, 1]));
    cache.putDetail("me", detail(3));
    cache.putSummary("me", summary("p"));
    cache.remember("me", {
      puuid: "p",
      name: { gameName: "暗夜里的光", tagLine: "10003" },
    });
    expect(cache.list("me", "p")?.page).toBe(2);
    expect(cache.detail("me", 3)?.gameId).toBe(3);
    expect(cache.summary("me", "p")?.puuid).toBe("p");
    expect(cache.list("other", "p"), "asked for as another account").toBeUndefined();

    cache.scope(null);
    expect(
      cache.list("me", "p"),
      "the client closed: the same account comes back to it",
    ).toBeDefined();
    cache.scope("other");
    expect(cache.viewer).toBe("other");
    cache.scope("me");
    expect(cache.list("me", "p"), "another account signed in between").toBeUndefined();
    expect(cache.detail("me", 3)).toBeUndefined();
    expect(cache.summary("me", "p")).toBeUndefined();

    // An answer that comes back after its account signed out is not kept.
    cache.putList("other", "p", list([1]));
    cache.putDetail("other", detail(1));
    cache.putSummary("other", summary("p"));
    cache.scope("other");
    expect(cache.list("other", "p")).toBeUndefined();
  });

  it("finds a player after the client closes", () => {
    const cache = new HistoryCache();
    cache.scope("me");
    cache.putSummary("me", {
      ...summary("p"),
      name: { gameName: "暗夜里的光", tagLine: "10003" },
    });
    cache.putList("me", "p", list([3, 2, 1]));
    cache.scope(null);

    expect(cache.find("me", "  暗夜里的光 # 10003 ")).toBe("p");
    expect(cache.find("other", "暗夜里的光#10003")).toBeUndefined();
  });

  it("empties for a cleanup and stays the same account's", () => {
    const cache = new HistoryCache();
    cache.scope("me");
    cache.putList("me", "p", list([3, 2, 1]));
    cache.putDetail("me", detail(3));
    cache.putSummary("me", summary("p"));
    cache.clear();
    expect(cache.list("me", "p")).toBeUndefined();
    expect(cache.detail("me", 3)).toBeUndefined();
    expect(cache.summary("me", "p")).toBeUndefined();
    expect(cache.find("me", "暗夜里的光#10003")).toBeUndefined();
    expect(cache.viewer).toBe("me");
    cache.putList("me", "p", list([1]));
    expect(cache.list("me", "p")?.games).toHaveLength(1);
  });

  it("follows the account the store's snapshot names, before anything is drawn", async () => {
    let emit: (event: Event) => void = () => undefined;
    const backend: Backend = {
      call: (async (command: string) =>
        command === "get_snapshot"
          ? { ...EMPTY_SNAPSHOT, rev: 1, me: me("a") }
          : command === "get_update_status"
            ? { state: "idle" }
            : null) as Backend["call"],
      onEvent: (handler) => {
        emit = handler;
        return () => undefined;
      },
      onResync: () => () => undefined,
      onUpdate: () => () => undefined,
      onHotkey: () => () => undefined,
    };
    const store = new AppStore(backend);
    await store.start();
    expect(store.history.viewer).toBe("a");
    store.history.putList("a", "p", list([1]));
    const patch = (rev: number, value: ReturnType<typeof me> | null): Event => ({
      type: "update",
      data: { rev, patch: { key: "me", value } } satisfies Update,
    });
    emit(patch(2, null));
    expect(store.history.list("a", "p"), "signed out, nothing else yet").toBeDefined();
    emit(patch(3, me("b")));
    expect(store.history.viewer).toBe("b");
    expect(store.history.list("a", "p")).toBeUndefined();
  });
});

function me(puuid: string) {
  return {
    puuid,
    name: null,
    level: 1,
    iconId: 0,
    ranked: { solo: null, flex: null },
  };
}
