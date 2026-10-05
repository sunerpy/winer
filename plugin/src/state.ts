// The core's state as the plugin sees it: the snapshot from the bridge's hello, kept current by
// patches, and the settings.
import type { Event, Patch, Settings, Snapshot } from "@winer/shared";

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
  }
}

export class PluginState {
  snapshot: Snapshot | null = null;
  settings: Settings | null = null;
  connected = false;
  readonly #listeners = new Set<() => void>();

  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  #changed(): void {
    for (const listener of this.#listeners) listener();
  }

  hello(snapshot: Snapshot, settings: Settings): void {
    this.snapshot = snapshot;
    this.settings = settings;
    this.#changed();
  }

  event(event: Event): void {
    if (event.type === "update" && this.snapshot && event.data.rev > this.snapshot.rev) {
      this.snapshot = { ...applyPatch(this.snapshot, event.data.patch), rev: event.data.rev };
      this.#changed();
    } else if (event.type === "settings") {
      this.settings = event.data;
      this.#changed();
    }
  }

  connection(connected: boolean): void {
    this.connected = connected;
    if (!connected) this.snapshot = null;
    this.#changed();
  }
}
