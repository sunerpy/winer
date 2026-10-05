// How each feat the core names (`view::Feat`, crates/core/src/view.rs) is drawn: its words, its
// glyph and its tone. The core already sorts a line's feats most telling first.
import type { Feat } from "@winer/shared";
import {
  Castle,
  Coins,
  Crown,
  Droplet,
  HandHelping,
  LogOut,
  type LucideIcon,
  Shield,
  Swords,
  Wheat,
  Zap,
} from "lucide-react";

import type { MessageKey } from "./i18n";

/** Multikills climb blue → violet → orange → gold, a spree is crimson, the game's leads are the
 *  scoreboard's amber, and going AFK is the danger red (DESIGN.md, Feats). */
export type FeatTone =
  | "double"
  | "triple"
  | "quadra"
  | "penta"
  | "legend"
  | "blood"
  | "best"
  | "away";

export interface FeatLook {
  name: MessageKey;
  why: MessageKey;
  tone: FeatTone;
  /** A glyph, or for a multikill its count, drawn as a numeral where there is no room for words. */
  icon: LucideIcon | 2 | 3 | 4 | 5;
}

export const FEATS: Record<Feat, FeatLook> = {
  afk: { name: "feat.afk", why: "feat.afkWhy", tone: "away", icon: LogOut },
  penta: { name: "feat.penta", why: "feat.pentaWhy", tone: "penta", icon: 5 },
  quadra: { name: "feat.quadra", why: "feat.quadraWhy", tone: "quadra", icon: 4 },
  legendary: { name: "feat.legendary", why: "feat.legendaryWhy", tone: "legend", icon: Crown },
  triple: { name: "feat.triple", why: "feat.tripleWhy", tone: "triple", icon: 3 },
  mostKills: { name: "feat.mostKills", why: "feat.mostKillsWhy", tone: "best", icon: Swords },
  mostDamage: { name: "feat.mostDamage", why: "feat.mostDamageWhy", tone: "best", icon: Zap },
  firstBlood: { name: "feat.firstBlood", why: "feat.firstBloodWhy", tone: "blood", icon: Droplet },
  mostTowers: { name: "feat.mostTowers", why: "feat.mostTowersWhy", tone: "best", icon: Castle },
  mostAssists: {
    name: "feat.mostAssists",
    why: "feat.mostAssistsWhy",
    tone: "best",
    icon: HandHelping,
  },
  mostGold: { name: "feat.mostGold", why: "feat.mostGoldWhy", tone: "best", icon: Coins },
  mostTaken: { name: "feat.mostTaken", why: "feat.mostTakenWhy", tone: "best", icon: Shield },
  mostCs: { name: "feat.mostCs", why: "feat.mostCsWhy", tone: "best", icon: Wheat },
  double: { name: "feat.double", why: "feat.doubleWhy", tone: "double", icon: 2 },
};

/** The core's order, most telling first. */
export const FEAT_ORDER = Object.keys(FEATS) as Feat[];
