// The window's mirror of the core: the live snapshot (patched by revision), the settings, the
// game-data catalog, the updater's and the hotkey's status and the recent notices.
import type {
  AugmentInfo,
  AssetInfo,
  ChampionInfo,
  GameData,
  HotkeyStatus,
  Notice,
  Patch,
  QueueInfo,
  Settings,
  Snapshot,
  Update,
  UpdateStatus,
} from "@winer/shared";
import { createContext, useContext, useEffect } from "react";

import type { Backend } from "./backend";
import { HistoryCache } from "./historyCache";
import { Observable, useObservable } from "./observable";

export const EMPTY_SNAPSHOT: Snapshot = {
  rev: 0,
  connection: { status: "searching" },
  me: null,
  phase: "None",
  champSelect: null,
  game: null,
  friends: null,
  lobby: null,
};

export function applyPatch(snapshot: Snapshot, patch: Patch): Snapshot {
  switch (patch.key) {
    case "connection":
      return { ...snapshot, connection: patch.value };
    case "me":
      return { ...snapshot, me: patch.value };
    case "phase":
      return { ...snapshot, phase: patch.value };
    case "champSelect":
      return { ...snapshot, champSelect: patch.value };
    case "game":
      return { ...snapshot, game: patch.value };
    case "friends":
      return { ...snapshot, friends: patch.value };
    case "lobby":
      return { ...snapshot, lobby: patch.value };
  }
}

/** A player's history asked for from inside the client; `id` tells two asks for one player apart. */
export interface HistoryRequest {
  puuid: string;
  id: number;
}

/** Game data by id, built once per catalog. */
export interface Catalog {
  champions: Map<number, ChampionInfo>;
  /** What pickers offer: each champion once, in the order of the name they show. */
  championList: ChampionInfo[];
  items: Map<number, AssetInfo>;
  spells: Map<number, AssetInfo>;
  perks: Map<number, AssetInfo>;
  augments: Map<number, AugmentInfo>;
  queues: Map<number, QueueInfo>;
}

export function catalogOf(data: GameData): Catalog {
  const byId = <T extends { id: number }>(list: T[]) =>
    new Map(list.map((entry) => [entry.id, entry]));
  return {
    champions: byId(data.champions),
    // The client also lists a mode's own copy of a champion under a namespaced alias
    // (`Jade_Annie`, 60001). Its games still need the copy's name and icon, but there is no second
    // champion to choose.
    championList: data.champions
      .filter((champion) => !champion.alias.includes("_"))
      .sort((a, b) => a.shortName.localeCompare(b.shortName, "zh-CN")),
    items: byId(data.items),
    spells: byId(data.spells),
    perks: byId(data.perks),
    augments: byId(data.augments),
    queues: byId(data.queues),
  };
}

export interface NoticeEntry extends Notice {
  id: number;
}

export class AppStore {
  readonly live = new Observable<Snapshot>(EMPTY_SNAPSHOT);
  readonly settings = new Observable<Settings | null>(null);
  readonly catalog = new Observable<Catalog | null>(null);
  readonly update = new Observable<UpdateStatus>({ state: "idle" });
  readonly notices = new Observable<readonly NoticeEntry[]>([]);
  /** What each augment does, by id; `null` until asked for (see `loadAugmentDetails`). */
  readonly augmentDetails = new Observable<ReadonlyMap<number, string> | null>(null);
  /** The global shortcut as the shell holds it; `null` until it has said. */
  readonly hotkey = new Observable<HotkeyStatus | null>(null);
  /** The latest history asked for from inside the client, which the shell's route follows. */
  readonly historyRequest = new Observable<HistoryRequest | null>(null);
  /** History: what the History page has shown, for the signed-in account (`historyCache.ts`). */
  readonly history = new HistoryCache();
  /** The read whose answer is still wanted; a reset makes an answer already on its way stale. */
  #augmentDetailsRead: Promise<void> | null = null;
  #noticeId = 0;
  #historyId = 0;
  /** Settings saves run one at a time, in the order they were made. */
  #queue: Promise<void> = Promise.resolve();
  #pending = 0;
  #confirmed: Settings | null = null;

  constructor(readonly backend: Backend) {}

  /** Subscribes first and reads second, so nothing that happens in between is lost: updates that
   *  arrive before the snapshot are held, then applied by revision. */
  async start(): Promise<() => void> {
    let held: Update[] | null = [];
    const offEvent = this.backend.onEvent((event) => {
      switch (event.type) {
        case "update":
          if (held) held.push(event.data);
          else this.apply(event.data);
          break;
        case "notice":
          this.pushNotice(event.data);
          break;
        case "settings":
          this.#confirm(event.data);
          break;
        case "gameData":
          void this.loadCatalog();
          break;
        case "openHistory":
          this.historyRequest.set({ puuid: event.data.puuid, id: ++this.#historyId });
          break;
      }
    });
    const offResync = this.backend.onResync(() => void this.resync());
    const offUpdate = this.backend.onUpdate((status) => this.update.set(status));
    const offHotkey = this.backend.onHotkey((status) => this.hotkey.set(status));

    const [snapshot, settings, update] = await Promise.all([
      this.backend.call("get_snapshot"),
      this.backend.call("get_settings"),
      this.backend.call("get_update_status"),
    ]);
    this.#setLive(snapshot);
    for (const pending of held) this.apply(pending);
    held = null;
    this.#confirm(settings);
    this.update.set(update);
    void this.loadCatalog();
    void this.loadHotkey();
    return () => {
      offEvent();
      offResync();
      offUpdate();
      offHotkey();
    };
  }

  /** The shortcut's state once at start; the shell announces every change after it. */
  async loadHotkey(): Promise<void> {
    try {
      const status = await this.backend.call("get_hotkey_status");
      if (status) this.hotkey.set(status);
    } catch {
      // Settings › 通用 then shows the shortcut without saying whether it took.
    }
  }

  apply(update: Update): void {
    const current = this.live.get();
    if (update.rev <= current.rev) return;
    this.#setLive({ ...applyPatch(current, update.patch), rev: update.rev });
  }

  async resync(): Promise<void> {
    this.#setLive(await this.backend.call("get_snapshot"));
    void this.loadCatalog();
  }

  /** Every live state goes through here. History: the cache is the signed-in account's before
   *  anything is drawn for it. */
  #setLive(snapshot: Snapshot): void {
    this.history.scope(snapshot.me?.puuid ?? null);
    this.live.set(snapshot);
  }

  async loadCatalog(): Promise<void> {
    const data = await this.backend.call("get_game_data");
    this.catalog.set(data ? catalogOf(data) : null);
  }

  /** Shows the change at once and saves it behind any save still on its way: two saves in flight
   *  could finish in either order on the core's blocking pool and leave the older one on disk.
   *  A refusal puts back what the core holds, unless a newer edit is still waiting. */
  saveSettings(next: Settings): Promise<void> {
    this.#confirmed ??= this.settings.get();
    this.settings.set(next);
    this.#pending += 1;
    const saved = this.#queue.then(() => this.backend.call("set_settings", { settings: next }));
    this.#queue = saved.then(
      () => undefined,
      () => undefined,
    );
    return saved.then(
      (settings) => {
        this.#pending -= 1;
        this.#confirm(settings);
      },
      (error: unknown) => {
        this.#pending -= 1;
        if (this.#pending === 0) this.settings.set(this.#confirmed);
        throw error;
      },
    );
  }

  /** Applies `change` to the newest settings, so two quick edits cannot overwrite each other. */
  async updateSettings(change: (settings: Settings) => Settings): Promise<void> {
    const current = this.settings.get();
    if (current) await this.saveSettings(change(current));
  }

  /** What the core holds. Shown unless edits of ours are still on their way: those are newer. */
  #confirm(settings: Settings): void {
    const before = this.#confirmed?.general;
    // The descriptions follow the switch and the language: read them again when either moves.
    if (
      before &&
      (before.augmentDetails !== settings.general.augmentDetails ||
        before.language !== settings.general.language)
    ) {
      this.#augmentDetailsRead = null;
      this.augmentDetails.set(null);
    }
    this.#confirmed = settings;
    if (this.#pending === 0) this.settings.set(settings);
  }

  /** Reads the augment descriptions once, the first time an augment is drawn. A failure leaves the
   *  names showing; the next augment drawn asks again. An answer that lands after the switch or the
   *  language moved is dropped: it describes what the user has just left. */
  loadAugmentDetails(): Promise<void> {
    if (this.augmentDetails.get()) return Promise.resolve();
    if (this.#augmentDetailsRead) return this.#augmentDetailsRead;
    const read: Promise<void> = this.backend
      .call("get_augment_details")
      .then((details) => {
        if (this.#augmentDetailsRead !== read) return;
        this.augmentDetails.set(new Map(details.map((detail) => [detail.id, detail.description])));
      })
      .catch(() => undefined)
      .finally(() => {
        if (this.#augmentDetailsRead === read) this.#augmentDetailsRead = null;
      });
    this.#augmentDetailsRead = read;
    return read;
  }

  pushNotice(notice: Notice): void {
    const entry = { ...notice, id: ++this.#noticeId };
    this.notices.update((list) => [entry, ...list].slice(0, 30));
  }
}

export const StoreContext = createContext<AppStore | null>(null);

export function useStore(): AppStore {
  const store = useContext(StoreContext);
  if (!store) throw new Error("useStore outside <StoreContext>");
  return store;
}

export function useLive<S>(select: (snapshot: Snapshot) => S): S {
  return useObservable(useStore().live, select);
}

/** Settings once loaded; the shell renders nothing before that, so pages may rely on it. */
export function useSettings(): Settings {
  const settings = useObservable(useStore().settings);
  if (!settings) throw new Error("settings are not loaded yet");
  return settings;
}

export function useCatalog(): Catalog | null {
  return useObservable(useStore().catalog);
}

export function useUpdateStatus(): UpdateStatus {
  return useObservable(useStore().update);
}

export function useNotices(): readonly NoticeEntry[] {
  return useObservable(useStore().notices);
}

export function useHotkeyStatus(): HotkeyStatus | null {
  return useObservable(useStore().hotkey);
}

/** The augment descriptions, asking for them the first time a component needs one. */
export function useAugmentDetails(): ReadonlyMap<number, string> | null {
  const store = useStore();
  const details = useObservable(store.augmentDetails);
  useEffect(() => {
    if (!details) void store.loadAugmentDetails();
  }, [details, store]);
  return details;
}
