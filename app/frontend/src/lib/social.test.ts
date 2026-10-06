// The window's half of the social features: the store's friends, lobby, history requests and
// hotkey status, and the recorder's reading of a keydown.
import type { Event, HotkeyStatus, LobbyView } from "@winer/shared";
import { describe, expect, it } from "vitest";

import type { Backend } from "./backend";
import { demoBackend, demoLobby } from "./demo";
import { keyOfCode, keycaps, record } from "./hotkey";
import { AppStore, EMPTY_SNAPSHOT, applyPatch } from "./store";

const press = (
  code: string,
  modifiers: Partial<Record<"ctrl" | "alt" | "shift" | "meta", boolean>>,
) => ({
  code,
  ctrlKey: Boolean(modifiers.ctrl),
  altKey: Boolean(modifiers.alt),
  shiftKey: Boolean(modifiers.shift),
  metaKey: Boolean(modifiers.meta),
});

describe("hotkey recorder", () => {
  it("names keys the way the core writes them", () => {
    expect(
      [
        "KeyW",
        "Digit7",
        "Numpad7",
        "F5",
        "F24",
        "ArrowUp",
        "Backquote",
        "NumpadAdd",
        "PageDown",
      ].map(keyOfCode),
    ).toEqual(["W", "7", "Num7", "F5", "F24", "Up", "Backquote", "NumAdd", "PageDown"]);
    expect(["Enter", "Escape", "Tab", "ShiftLeft", "F25", "NumpadEnter"].map(keyOfCode)).toEqual([
      null,
      null,
      null,
      null,
      null,
      null,
    ]);
  });

  it("waits for a key while only modifiers are held and needs Ctrl, Alt or Win", () => {
    expect(record(press("ShiftLeft", { ctrl: true, shift: true }))).toEqual({
      kind: "partial",
      keys: ["Ctrl", "Shift"],
    });
    expect(record(press("KeyW", { ctrl: true, shift: true }))).toEqual({
      kind: "done",
      combo: "Ctrl+Shift+W",
    });
    expect(record(press("ArrowLeft", { meta: true, alt: true }))).toEqual({
      kind: "done",
      combo: "Alt+Super+Left",
    });
    expect(record(press("KeyW", { shift: true })), "Shift and a letter is typing").toEqual({
      kind: "invalid",
    });
    expect(record(press("Enter", { ctrl: true }))).toEqual({ kind: "invalid" });
  });

  it("shows a combination as the keys printed on the keyboard", () => {
    expect(keycaps("Ctrl+Alt+Super+Slash")).toEqual(["Ctrl", "Alt", "Win", "/"]);
    expect(keycaps("Ctrl+Up")).toEqual(["Ctrl", "↑"]);
  });
});

describe("store", () => {
  it("patches the friends and the lobby like every other field", () => {
    const lobby: LobbyView = demoLobby();
    const withLobby = applyPatch(EMPTY_SNAPSHOT, { key: "lobby", value: lobby });
    expect(withLobby.lobby).toBe(lobby);
    const withFriends = applyPatch(withLobby, { key: "friends", value: { friends: [] } });
    expect(withFriends).toMatchObject({ lobby, friends: { friends: [] } });
    expect(applyPatch(withFriends, { key: "lobby", value: null }).lobby).toBeNull();
  });

  it("turns each history the client asks for into a request, the same player twice included", async () => {
    const base = demoBackend();
    let push: (event: Event) => void = () => undefined;
    const store = new AppStore({
      ...base,
      onEvent: (handler) => {
        push = handler;
        return () => undefined;
      },
    });
    await store.start();
    push({ type: "openHistory", data: { puuid: "demo-3" } });
    const first = store.historyRequest.get();
    expect(first?.puuid).toBe("demo-3");
    push({ type: "openHistory", data: { puuid: "demo-3" } });
    expect(store.historyRequest.get()?.id).not.toBe(first?.id);
  });

  it("reads the hotkey's state at start and follows the shell's announcements", async () => {
    let announce: (status: HotkeyStatus) => void = () => undefined;
    const base = demoBackend();
    const backend: Backend = {
      ...base,
      onHotkey: (handler) => {
        announce = handler;
        return () => undefined;
      },
    };
    const store = new AppStore(backend);
    await store.start();
    await store.loadHotkey();
    expect(store.hotkey.get()).toMatchObject({ shortcut: "Alt+Backquote", active: true });
    announce({
      shortcut: "Alt+Backquote",
      active: false,
      suspended: false,
      error: "taken",
      callout: { shortcut: null, active: false, error: null },
    });
    expect(store.hotkey.get()?.error).toBe("taken");
  });
});
