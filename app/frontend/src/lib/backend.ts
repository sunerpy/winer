// The one seam between the window and the core: every command, typed end to end, and the four
// event channels. The Tauri implementation is the product; `demo.ts` is the browser preview.
import { invoke } from "@tauri-apps/api/core";
import { type UnlistenFn, listen } from "@tauri-apps/api/event";
import type {
  AppInfo,
  Audience,
  AugmentDetail,
  BackupChannel,
  BackupInfo,
  Build,
  CalloutRule,
  ChallengeProfile,
  Event,
  GameData,
  General,
  HotkeyStatus,
  IpcError,
  LoadoutSummary,
  MatchDetail,
  MatchPage,
  Mode,
  PageOutcome,
  PlayerProfile,
  PlayerSummary,
  PluginStatus,
  Position,
  Presence,
  RunePage,
  Settings,
  SkinChoice,
  Snapshot,
  UpdateStatus,
} from "@winer/shared";

type Command<Args, Result> = { args: Args; result: Result };

export interface Commands {
  get_snapshot: Command<undefined, Snapshot>;
  get_settings: Command<undefined, Settings>;
  set_settings: Command<{ settings: Settings }, Settings>;
  get_game_data: Command<undefined, GameData | null>;
  get_match_history: Command<{ puuid: string; begin: number; count: number }, MatchPage>;
  get_match_detail: Command<{ gameId: number }, MatchDetail>;
  get_augment_details: Command<undefined, AugmentDetail[]>;
  find_player: Command<{ riotId: string }, PlayerProfile>;
  get_player_summary: Command<{ puuid: string }, PlayerSummary>;
  get_presence: Command<undefined, Presence>;
  set_availability: Command<{ availability: string }, null>;
  set_status_message: Command<{ message: string }, null>;
  restart_client_ui: Command<undefined, null>;
  send_callout: Command<{ audience: Audience | null }, number>;
  preview_callout: Command<{ rule: CalloutRule; general: General }, string[]>;
  bench_swap: Command<{ championId: number }, null>;
  reroll: Command<undefined, null>;
  get_plugin_status: Command<undefined, PluginStatus>;
  enable_plugin: Command<undefined, PluginStatus>;
  disable_plugin: Command<undefined, PluginStatus>;
  get_app_info: Command<undefined, AppInfo>;
  relaunch_elevated: Command<undefined, null>;
  reveal_logs: Command<undefined, null>;
  get_autostart: Command<undefined, boolean>;
  set_autostart: Command<{ enabled: boolean }, boolean>;
  get_update_status: Command<undefined, UpdateStatus>;
  check_update: Command<undefined, UpdateStatus>;
  install_update: Command<undefined, null>;
  open_releases: Command<undefined, null>;
  open_docs: Command<{ page: "home" | "rating" }, null>;
  // The profile tools on the Tools page.
  get_skins: Command<undefined, SkinChoice[]>;
  get_profile_background: Command<undefined, number | null>;
  /** Answers the background the client reports afterwards: the old one where it refused. */
  set_profile_background: Command<{ skinId: number }, number | null>;
  get_challenge_profile: Command<undefined, ChallengeProfile>;
  set_challenge_profile: Command<
    { challengeIds: number[]; titleId: number | null },
    ChallengeProfile
  >;
  get_game_settings_backups: Command<undefined, BackupInfo[]>;
  create_game_settings_backup: Command<undefined, BackupInfo>;
  restore_game_settings_backup: Command<{ id: number; channels: BackupChannel[] }, null>;
  delete_game_settings_backup: Command<{ id: number }, null>;
  import_game_settings_backup: Command<{ text: string }, BackupInfo>;
  reveal_game_settings_backup: Command<{ id: number }, null>;
  // Social.
  get_hotkey_status: Command<undefined, HotkeyStatus>;
  suspend_hotkey: Command<{ suspended: boolean }, HotkeyStatus>;
  // Runes, spells, builds and item sets.
  get_build: Command<{ championId: number; mode: Mode; lane: Position | null }, Build>;
  apply_runes: Command<{ championId: number; page: RunePage }, PageOutcome>;
  apply_spells: Command<{ spells: [number, number] }, null>;
  write_item_set: Command<{ championId: number; mode: Mode; lane: Position | null }, null>;
  clear_item_sets: Command<undefined, number>;
  get_loadout_summary: Command<undefined, LoadoutSummary>;
  clear_loadouts: Command<undefined, LoadoutSummary>;
}

export type CommandName = keyof Commands;
export type ArgsOf<K extends CommandName> = Commands[K]["args"] extends undefined
  ? []
  : [Commands[K]["args"]];

export interface Backend {
  call<K extends CommandName>(command: K, ...args: ArgsOf<K>): Promise<Commands[K]["result"]>;
  /** Every core event. Returns the unsubscribe. */
  onEvent(handler: (event: Event) => void): () => void;
  /** The window missed events and must read the snapshot again. */
  onResync(handler: () => void): () => void;
  onUpdate(handler: (status: UpdateStatus) => void): () => void;
  /** The global shortcut, as the shell holds it after each change. */
  onHotkey(handler: (status: HotkeyStatus) => void): () => void;
}

/** `UnlistenFn` is typed `() => void` but implemented async; a failure can arrive either way, and
 *  an exception escaping a React cleanup would unmount the whole tree. */
function unlistenQuietly(unlisten: UnlistenFn): void {
  try {
    void Promise.resolve(unlisten()).catch(() => undefined);
  } catch {
    // Contained on purpose: the listener dies with the webview anyway.
  }
}

function subscribe<T>(name: string, handler: (payload: T) => void): () => void {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;
  listen<T>(name, (event) => handler(event.payload))
    .then((fn) => {
      if (disposed) unlistenQuietly(fn);
      else unlisten = fn;
    })
    .catch(() => undefined);
  return () => {
    disposed = true;
    if (unlisten) unlistenQuietly(unlisten);
  };
}

export const tauriBackend: Backend = {
  call: (command, ...args) => invoke(command, args[0]),
  onEvent: (handler) => subscribe<Event>("winer://event", handler),
  onResync: (handler) => subscribe<null>("winer://resync", () => handler()),
  onUpdate: (handler) => subscribe<UpdateStatus>("winer://update", handler),
  onHotkey: (handler) => subscribe<HotkeyStatus>("winer://hotkey", handler),
};

function isIpcError(error: unknown): error is IpcError {
  return typeof error === "object" && error !== null && "code" in error && "message" in error;
}

/** The message to show for a failed call, whatever shape the failure took. */
export function errorMessage(error: unknown): string {
  if (isIpcError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function errorCode(error: unknown): IpcError["code"] | null {
  return isIpcError(error) ? error.code : null;
}
