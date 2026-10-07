// History › 英雄: each champion's games among those loaded, the most played first. The numbers are
// the list's own: the same filter, custom games hidden or not, remakes left out.
import { relativeTime } from "@winer/shared";

import { ChampionIcon } from "../../game/icons";
import { KdaValue, WinRate } from "../../game/stats";
import { useLanguage, useT } from "../../lib/i18n";
import { useCatalog } from "../../lib/store";
import { Card } from "../../ui";
import type { ChampionRow } from "./records";

export function ChampionTable({ rows }: { rows: readonly ChampionRow[] }) {
  const t = useT();
  const language = useLanguage();
  const catalog = useCatalog();
  const now = Date.now();
  const head = "px-2 py-1.5 text-left text-[11px] font-medium text-fg-subtle";
  const cell = "px-2 py-1.5";
  return (
    <Card className="overflow-x-auto">
      <table className="w-full min-w-[560px] border-collapse text-[12.5px]">
        <caption className="sr-only">{t("history.champions")}</caption>
        <thead>
          <tr className="border-b border-border">
            <th scope="col" className={head}>
              {t("history.champ.champion")}
            </th>
            <th scope="col" className={head}>
              {t("history.champ.games")}
            </th>
            <th scope="col" className={head}>
              {t("history.champ.winRate")}
            </th>
            <th scope="col" className={head}>
              KDA
            </th>
            <th scope="col" className={head}>
              {t("history.champ.score")}
            </th>
            <th scope="col" className={head}>
              MVP / SVP
            </th>
            <th scope="col" className={head}>
              {t("history.champ.last")}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.championId} className="border-b border-border last:border-b-0">
              <th scope="row" className={`${cell} text-left font-medium text-fg`}>
                <span className="flex items-center gap-2">
                  <ChampionIcon id={row.championId} size={24} />
                  {catalog?.champions.get(row.championId)?.name ?? `#${row.championId}`}
                </span>
              </th>
              <td className={`${cell} mono`}>{row.games}</td>
              <td className={cell}>
                <WinRate wins={row.wins} games={row.games} />
              </td>
              <td className={cell}>
                <KdaValue kills={row.kills} deaths={row.deaths} assists={row.assists} />
              </td>
              <td className={`${cell} mono`}>{row.score === null ? "—" : row.score.toFixed(1)}</td>
              <td className={`${cell} mono`}>
                {row.mvp} / {row.svp}
              </td>
              <td className={`${cell} text-fg-muted`}>{relativeTime(row.last, now, language)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}
