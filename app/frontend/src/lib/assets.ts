// Game-data images through the shell's `lcu` protocol, which forwards to the connected client.
import { currentPlatform } from "./platform";

/** WebView2 serves custom protocols as `http://<scheme>.localhost`; WebKit keeps `<scheme>://`. */
const BASE = currentPlatform() === "windows" ? "http://lcu.localhost" : "lcu://localhost";

export function assetUrl(path: string | undefined | null): string | undefined {
  return path ? `${BASE}${path.startsWith("/") ? path : `/${path}`}` : undefined;
}

export function championIconUrl(id: number): string | undefined {
  return id > 0 ? assetUrl(`/lol-game-data/assets/v1/champion-icons/${id}.png`) : undefined;
}

export function profileIconUrl(id: number): string | undefined {
  return id > 0 ? assetUrl(`/lol-game-data/assets/v1/profile-icons/${id}.jpg`) : undefined;
}
