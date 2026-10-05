// The plugin's line to the desktop app. The app writes `bootstrap.json` beside this module with the
// bridge's port and a per-session token; the plugin reads it, connects, and reconnects whenever the
// app restarts (new port, new token) or is not running yet.
import type {
  BridgeMessage,
  Event,
  LogLevel,
  PluginMessage,
  Settings,
  Snapshot,
} from "@winer/shared";

export interface Bootstrap {
  port: number;
  token: string;
}

export function parseBootstrap(value: unknown): Bootstrap | null {
  if (typeof value !== "object" || value === null) return null;
  const { port, token } = value as Record<string, unknown>;
  if (typeof port !== "number" || !Number.isInteger(port) || port < 1 || port > 65_535) return null;
  if (typeof token !== "string" || token.length === 0) return null;
  return { port, token };
}

export interface BridgeHandlers {
  onHello(snapshot: Snapshot, settings: Settings): void;
  onEvent(event: Event): void;
  onConnection(connected: boolean): void;
}

/** Waits between attempts: quick at first, then every ten seconds while the app is away. */
export function backoff(attempt: number): number {
  return Math.min(10_000, 1000 * 2 ** Math.min(attempt, 4));
}

export class Bridge {
  #socket: WebSocket | null = null;
  #attempt = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #stopped = false;

  constructor(
    private readonly handlers: BridgeHandlers,
    private readonly identity: { version: string; context: string },
    private readonly bootstrapUrl = new URL("bootstrap.json", import.meta.url).toString(),
  ) {}

  start(): void {
    void this.#connect();
  }

  stop(): void {
    this.#stopped = true;
    clearTimeout(this.#timer);
    this.#socket?.close();
  }

  log(level: LogLevel, message: string): void {
    this.#send({ type: "log", level, message });
  }

  /** Asks winer to take `championId` off the bench. False while there is no connection. */
  benchSwap(championId: number): boolean {
    if (this.#socket?.readyState !== WebSocket.OPEN) return false;
    this.#send({ type: "benchSwap", championId });
    return true;
  }

  #send(message: PluginMessage): void {
    if (this.#socket?.readyState === WebSocket.OPEN) this.#socket.send(JSON.stringify(message));
  }

  #retry(): void {
    if (this.#stopped) return;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.#connect(), backoff(this.#attempt++));
  }

  async #connect(): Promise<void> {
    if (this.#stopped) return;
    let bootstrap: Bootstrap | null = null;
    try {
      const response = await fetch(this.bootstrapUrl, { cache: "no-store" });
      bootstrap = response.ok ? parseBootstrap(await response.json()) : null;
    } catch {
      bootstrap = null;
    }
    if (!bootstrap) return this.#retry();

    const socket = new WebSocket(
      `ws://127.0.0.1:${bootstrap.port}/?token=${encodeURIComponent(bootstrap.token)}`,
    );
    this.#socket = socket;
    socket.addEventListener("open", () => {
      this.#attempt = 0;
      this.#send({ type: "hello", ...this.identity });
      this.handlers.onConnection(true);
    });
    socket.addEventListener("message", (message) => {
      let data: BridgeMessage;
      try {
        data = JSON.parse(String(message.data)) as BridgeMessage;
      } catch {
        return;
      }
      if (data.type === "hello") this.handlers.onHello(data.snapshot, data.settings);
      else this.handlers.onEvent(data.event);
    });
    socket.addEventListener("close", () => {
      if (this.#socket === socket) this.#socket = null;
      this.handlers.onConnection(false);
      this.#retry();
    });
  }
}
