// Champ select on the Rift: champions worth considering for the local player's pick, best first,
// each with why. Shown only: winer hovers or picks nothing for them; the pick stays the player's.
import type { RecommendReason, Recommendation } from "@winer/shared";

import { ChampionIcon } from "../../game/icons";
import { type Translate, useT } from "../../lib/i18n";
import { type Catalog, useCatalog } from "../../lib/store";
import { Badge, Card, type Tone } from "../../ui";

function reasonText(reason: RecommendReason, t: Translate, catalog: Catalog | null): string {
  const champion = (id: number) => catalog?.champions.get(id)?.shortName ?? `#${id}`;
  const rate = (win: number) => `${Math.round(win * 100)}%`;
  switch (reason.kind) {
    case "counters":
      return t("suggest.counters", {
        champion: champion(reason.championId),
        win: rate(reason.win),
      });
    case "counteredBy":
      return t("suggest.counteredBy", {
        champion: champion(reason.championId),
        win: rate(reason.win),
      });
    case "tier":
      return t("suggest.tier", { tier: reason.tier });
    case "played":
      return t("suggest.played", { games: reason.games, wins: reason.wins });
    case "inPickList":
      return t("suggest.inPickList");
    case "noData":
      return t("suggest.noData");
  }
}

function reasonTone(reason: RecommendReason): Tone {
  if (reason.kind === "counters") return "win";
  if (reason.kind === "counteredBy") return "loss";
  return "neutral";
}

export function Suggestions({ items }: { items: readonly Recommendation[] }) {
  const t = useT();
  const catalog = useCatalog();
  return (
    <Card padding="sm" className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <span className="flex flex-col gap-0.5">
        <span className="eyebrow">{t("suggest.title")}</span>
        <span className="text-[11px] text-fg-subtle">{t("suggest.hint")}</span>
      </span>
      <ol
        aria-label={t("suggest.title")}
        className="flex min-w-0 flex-wrap items-center gap-x-5 gap-y-2"
      >
        {items.map((item) => (
          <li key={item.championId} className="flex min-w-0 items-center gap-2">
            <ChampionIcon id={item.championId} size={32} />
            <span className="flex min-w-0 flex-col gap-1">
              <span className="text-[13px] font-medium text-fg">
                {catalog?.champions.get(item.championId)?.name ?? `#${item.championId}`}
              </span>
              <span className="flex flex-wrap gap-1">
                {item.reasons.map((reason, index) => (
                  <Badge key={index} tone={reasonTone(reason)}>
                    {reasonText(reason, t, catalog)}
                  </Badge>
                ))}
              </span>
            </span>
          </li>
        ))}
      </ol>
    </Card>
  );
}
