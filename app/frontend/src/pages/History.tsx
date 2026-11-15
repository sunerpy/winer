import { riotId, type ExportFormat, type HistorySource, type MatchSummary } from "@winer/shared";
import { Download, Search, UserRound } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";

import { FormLabel, StandingRule } from "../game/FormRules";
import { MatchDetailView } from "../game/MatchDetailView";
import { MatchRow } from "../game/MatchRow";
import { ProfileIcon } from "../game/icons";
import { FormLine, RankBadge, ResultStrip, StreakBadge, TierBadge, TitleChip } from "../game/stats";
import { NoteEditor } from "../game/notes";
import { errorCode, errorMessage } from "../lib/backend";
import { useCached } from "../lib/historyCache";
import { useT } from "../lib/i18n";
import { useCatalog, useLive, useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { useShell } from "../shell/navigation";
import {
  Button,
  Card,
  EmptyState,
  ErrorNote,
  Input,
  Pager,
  Segmented,
  Skeleton,
  Spinner,
  Toggle,
  toast,
} from "../ui";
import { PageBody } from "./common";
import { ChampionTable } from "./history/ChampionTable";
import { historyFailure } from "./history/failure";
import { type ExportWords, championRows, exportStem, gamesCsv, gamesJson } from "./history/records";

const PAGE_SIZES = ["10", "15", "25", "50"] as const;
type PageSize = (typeof PAGE_SIZES)[number];
const PAGE_SIZE_KEY = "winer.history.pageSize";
/** Games asked for at a time, the most one request returns. */
const CHUNK = 50;
const HISTORY_REGIONS = [
  ["HN1", "history.region.hn1"],
  ["HN10", "history.region.hn10"],
  ["TJ100", "history.region.tj100"],
  ["TJ101", "history.region.tj101"],
  ["NJ100", "history.region.nj100"],
  ["GZ100", "history.region.gz100"],
  ["CQ100", "history.region.cq100"],
  ["BGP2", "history.region.bgp2"],
] as const;

/** `name#tag` with both halves, trimmed; `null` for anything less. */
function fullRiotId(input: string): string | null {
  const at = input.lastIndexOf("#");
  if (at < 0) return null;
  const [name, tag] = [input.slice(0, at).trim(), input.slice(at + 1).trim()];
  return name && tag ? `${name}#${tag}` : null;
}

/** A region's name, `HN1` → 艾欧尼亚; the id itself for one winer does not know. */
function regionName(t: ReturnType<typeof useT>, platformId: string): string {
  const known = HISTORY_REGIONS.find(([id]) => id === platformId.trim().toUpperCase());
  return known ? t(known[1]) : platformId;
}

/** Where a search lands: the player, the region it was found on (none for this one), the regions
 *  holding the account, and the Riot ID searched for. */
interface Found {
  puuid: string;
  platformId?: string;
  regions?: string[];
  riotId?: string;
}

function regionArgs(platformId?: string): { platformId?: string } {
  return platformId ? { platformId } : {};
}

function readPageSize(): PageSize {
  try {
    const saved = localStorage.getItem(PAGE_SIZE_KEY);
    return PAGE_SIZES.find((size) => size === saved) ?? "10";
  } catch {
    return "10";
  }
}

function savePageSize(size: PageSize): void {
  try {
    localStorage.setItem(PAGE_SIZE_KEY, size);
  } catch {
    // A view preference only; losing it costs one click.
  }
}

function HistoryPager({
  page,
  pages,
  more,
  onPage,
}: {
  page: number;
  pages: number;
  more: boolean;
  onPage: (page: number) => void;
}) {
  const t = useT();
  const last = Math.max(1, pages);
  const [draft, setDraft] = useState(String(page));
  useEffect(() => setDraft(String(page)), [page]);
  const jump = () => {
    const wanted = Number.parseInt(draft, 10);
    if (Number.isFinite(wanted)) onPage(Math.min(last, Math.max(1, wanted)));
  };
  return (
    <div
      role="navigation"
      aria-label={t("history.pages")}
      className="flex flex-wrap items-center gap-1.5"
    >
      <Button size="sm" variant="ghost" disabled={page <= 1} onClick={() => onPage(1)}>
        {t("history.first")}
      </Button>
      <Pager
        page={page}
        known={pages}
        more={more}
        onPage={onPage}
        labels={{
          nav: t("history.pageNumbers"),
          previous: t("history.previous"),
          next: t("history.next"),
          page: (n) => t("history.page", { n }),
        }}
      />
      <Button
        size="sm"
        variant="ghost"
        disabled={more || page >= last}
        onClick={() => onPage(last)}
      >
        {t("history.last")}
      </Button>
      <form
        className="ml-1 flex items-center gap-1"
        onSubmit={(event) => {
          event.preventDefault();
          jump();
        }}
      >
        <Input
          type="number"
          inputMode="numeric"
          min={1}
          max={last}
          value={draft}
          disabled={more}
          aria-label={t("history.jump")}
          className="w-16"
          onChange={(event) => setDraft(event.target.value)}
        />
        <Button type="submit" size="sm" disabled={more}>
          {t("history.go")}
        </Button>
      </form>
    </div>
  );
}

const FILTERS = ["all", "ranked", "normal", "aram", "other"] as const;
type Filter = (typeof FILTERS)[number];

const QUEUES: Record<Exclude<Filter, "all" | "other">, number[]> = {
  ranked: [420, 440],
  normal: [400, 430, 480, 490],
  aram: [450, 100, 2400],
};

function matches(filter: Filter, game: MatchSummary): boolean {
  if (filter === "all") return true;
  if (filter === "other") return !Object.values(QUEUES).some((ids) => ids.includes(game.queueId));
  return QUEUES[filter].includes(game.queueId);
}

/** The other regions holding the account shown, one press each to see it there. */
function AlsoIn({
  regions,
  showing,
  onRegion,
}: {
  regions?: string[];
  /** The region shown now, which is not one to go to. */
  showing: string;
  onRegion: (region: string) => void;
}) {
  const t = useT();
  const others = (regions ?? []).filter(
    (region) => region.toUpperCase() !== showing.trim().toUpperCase(),
  );
  if (others.length === 0) return null;
  return (
    <span
      role="group"
      aria-label={t("history.alsoIn")}
      className="flex flex-wrap items-center gap-1.5 text-[12px] text-fg-subtle"
    >
      <span aria-hidden>{t("history.alsoIn")}</span>
      {others.map((region) => (
        <Button key={region} size="sm" variant="outline" onClick={() => onRegion(region)}>
          {regionName(t, region)}
        </Button>
      ))}
    </span>
  );
}

function PlayerHeader({
  puuid,
  viewer,
  connected,
  platformId,
  current,
  regions,
  isMe,
  onMine,
  onRegion,
}: {
  puuid: string;
  /** The signed-in account, whose cache this is. */
  viewer: string;
  connected: boolean;
  platformId?: string;
  /** The connected client's own region, empty when it is not known. */
  current: string;
  /** The regions holding this account, from the search that found it. */
  regions?: string[];
  isMe: boolean;
  onMine: () => void;
  /** Shows the account from `region`: the connected one's own path where it is that one. */
  onRegion: (region: string) => void;
}) {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  // Drawn at once on a revisit, then read again: a game may have ended since.
  const summary = useCached(
    store.history.summary(viewer, puuid, platformId),
    () => store.backend.call("get_player_summary", { puuid, ...regionArgs(platformId) }),
    (value) => store.history.putSummary(viewer, value, platformId),
    { refresh: connected, deps: [puuid, platformId], enabled: connected },
  );
  // The tier and its words follow the rating settings; asked once the record is in, so the core
  // reads the player once.
  const { tiers, customTiers } = settings.automation.callout;
  const { language, titles } = settings.general;
  const standing = useAsync(
    () => store.backend.call("get_player_standing", { puuid, ...regionArgs(platformId) }),
    [puuid, platformId, tiers, customTiers.join("\n"), language, titles],
    connected && summary.data !== undefined,
  );
  const player = summary.data;
  const rating = standing.data?.rating ?? null;
  return (
    <>
      <Card className="flex flex-wrap items-center gap-x-6 gap-y-3">
        {player ? (
          <>
            <span className="flex items-center gap-3">
              <ProfileIcon id={player.iconId} size={48} />
              <span className="flex flex-col">
                <span className="text-[16px] font-semibold text-fg">{riotId(player.name)}</span>
                <span className="mono text-[11.5px] text-fg-subtle">
                  {t("common.level", { level: player.level })}
                  {player.private && ` · ${t("common.private")}`}
                </span>
              </span>
            </span>
            <AlsoIn regions={regions} showing={platformId ?? current} onRegion={onRegion} />
            {platformId ? (
              <span className="text-[12px] text-fg-subtle">{t("history.crossRegionRanks")}</span>
            ) : (
              <>
                <span className="flex flex-col gap-1">
                  <span className="text-[11px] text-fg-subtle">{t("common.solo")}</span>
                  <RankBadge rank={player.ranked.solo} />
                </span>
                <span className="flex flex-col gap-1">
                  <span className="text-[11px] text-fg-subtle">{t("common.flex")}</span>
                  <RankBadge rank={player.ranked.flex} />
                </span>
              </>
            )}
            {player.recent.games > 0 && (
              <span className="flex flex-col gap-1">
                <span className="text-[11px] text-fg-subtle">
                  <FormLabel form={player.recent} scope={standing.data?.scope ?? null} />
                </span>
                <span className="flex items-center gap-2">
                  <FormLine form={player.recent} />
                  <StreakBadge streak={player.recent.streak} />
                </span>
                <ResultStrip matches={player.recent.matches} limit={50} dense />
              </span>
            )}
            {standing.data && rating && (
              <span className="flex min-w-0 flex-col gap-1">
                <span className="inline-flex items-center gap-1 text-[11px] text-fg-subtle">
                  {t("history.standing")}
                  <StandingRule standing={standing.data} form={player.recent} />
                </span>
                <span className="flex min-w-0 items-center gap-1.5">
                  <TierBadge rating={rating} />
                  {rating.title && <TitleChip name={rating.title} />}
                </span>
                {rating.quip && (
                  <span title={rating.quip} className="truncate text-[11px] text-fg-subtle">
                    {t("live.quip", { quip: rating.quip })}
                  </span>
                )}
              </span>
            )}
          </>
        ) : summary.loading ? (
          <span className="flex items-center gap-3">
            <Skeleton className="size-12 rounded-full!" />
            <span className="flex flex-col gap-2">
              <Skeleton className="h-4 w-40" />
              <Skeleton className="h-3 w-24" />
            </span>
          </span>
        ) : (
          <ErrorNote
            {...historyFailure(summary.error, t)}
            retryLabel={t("common.retry")}
            onRetry={summary.reload}
          />
        )}
        {!isMe && (
          <Button size="sm" icon={UserRound} className="ml-auto" onClick={onMine}>
            {t("history.mine")}
          </Button>
        )}
      </Card>
      {connected && !isMe && player && <NoteEditor puuid={puuid} name={player.name} />}
    </>
  );
}

function SearchBar({
  viewer,
  connected,
  onFound,
}: {
  viewer: string;
  connected: boolean;
  onFound: (found: Found) => void;
}) {
  const t = useT();
  const store = useStore();
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [invalid, setInvalid] = useState(false);

  /** A player looked up before, on this region or another. */
  const cached = (): Found | undefined => {
    for (const platformId of [undefined, ...HISTORY_REGIONS.map(([id]) => id)]) {
      const puuid = store.history.find(viewer, query, platformId);
      if (puuid) return { puuid, platformId };
    }
    return undefined;
  };

  const submit = async () => {
    if (!query.trim()) return;
    setBusy(true);
    setInvalid(false);
    const riotId = fullRiotId(query) ?? undefined;
    try {
      if (!connected) {
        const kept = cached();
        if (!kept) throw new Error(t("history.offlineMissing"));
        onFound(kept);
        return;
      }
      // No region asked for: the connected one first, then wherever the account plays.
      const player = await store.backend.call("find_player", { riotId: query });
      const platformId = player.region ?? undefined;
      store.history.remember(viewer, player, platformId);
      onFound({
        puuid: player.puuid,
        platformId,
        regions: platformId ? [platformId, ...player.otherRegions] : undefined,
        riotId,
      });
    } catch (error) {
      const kept = cached();
      if (kept) {
        onFound(kept);
        return;
      }
      setInvalid(true);
      toast(
        !connected
          ? t("history.offlineMissing")
          : errorCode(error) === "notFound"
            ? t(riotId ? "history.notFoundAnywhere" : "history.notFound")
            : errorMessage(error),
        "danger",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      role="search"
      className="flex flex-wrap items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <Input
        icon={Search}
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setInvalid(false);
        }}
        placeholder={t("history.search")}
        aria-label={t("history.search")}
        invalid={invalid}
        className="w-[320px]"
      />
      <Button type="submit" variant="primary" loading={busy}>
        {t("history.find")}
      </Button>
    </form>
  );
}

interface Loaded {
  /** Every game so far, newest first; pages are cut from these. */
  games: MatchSummary[];
  /** Where the next request starts. */
  next: number;
  more: boolean;
  loading: boolean;
  error: unknown;
  source: HistorySource | null;
  /** Drawn from the cache: the newest games are asked for again before anything else. */
  stale: boolean;
  /** The last page held fewer games than asked while more remain: the server is too busy for
   *  whole pages, so the rest is read only as far as the list is looked at. */
  short: boolean;
}

const NOTHING_YET: Loaded = {
  games: [],
  next: 0,
  more: true,
  loading: false,
  error: null,
  source: null,
  stale: false,
  short: false,
};

function GameList({
  puuid,
  viewer,
  connected,
  platformId,
}: {
  puuid: string;
  viewer: string;
  connected: boolean;
  platformId?: string;
}) {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const hideCustom = useSettings().history.hideCustomGames;
  // Where the list was left on the last visit, if it was this account's.
  const [kept] = useState(() => store.history.list(viewer, puuid, platformId));
  const [loaded, setLoaded] = useState<Loaded>(() =>
    kept ? { ...NOTHING_YET, ...kept, stale: true } : NOTHING_YET,
  );
  const [filter, setFilter] = useState<Filter>(
    () => FILTERS.find((value) => value === kept?.filter) ?? "all",
  );
  const [size, setSize] = useState<PageSize>(readPageSize);
  const [page, setPage] = useState(kept?.page ?? 1);
  const [view, setView] = useState<"games" | "champions">("games");
  const catalog = useCatalog();
  const [selected, setSelected] = useState<number | null>(null);
  const top = useRef<HTMLDivElement>(null);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  const toggleId = useId();
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const perPage = Number(size);
  const shown = useMemo(
    () =>
      loaded.games.filter(
        (game) => matches(filter, game) && !(hideCustom && game.kind === "custom"),
      ),
    [loaded.games, filter, hideCustom],
  );
  const hidden = hideCustom
    ? loaded.games.filter((game) => game.kind === "custom" && matches(filter, game)).length
    : 0;
  const known = Math.ceil(shown.length / perPage);
  // Past the last page there is nothing to show; the last one stands in.
  const current = loaded.more ? page : Math.min(page, Math.max(1, known));
  const visible = shown.slice((current - 1) * perPage, current * perPage);
  // While the server sends short pages, reading on waits until the page in view or the champion
  // view needs more games: reading every page then takes hundreds of requests, which compete with
  // champ select's for the same busy server.
  const paused = loaded.short && view === "games" && shown.length >= current * perPage;
  const filling = connected && loaded.more && !paused;
  const champions = useMemo(() => (loaded.more ? [] : championRows(shown)), [loaded.more, shown]);

  // The games as shown (filter, custom games) into a file in Downloads, the player's own lines.
  const exportGames = async (format: ExportFormat) => {
    const words: ExportWords = {
      header: t("history.exportHeader").split(","),
      champion: (id) => catalog?.champions.get(id)?.name ?? String(id),
      queue: (game) => catalog?.queues.get(game.queueId)?.name ?? game.gameMode,
      result: (game) =>
        game.line.remake
          ? t("history.result.remake")
          : game.line.win
            ? t("history.result.win")
            : t("history.result.loss"),
    };
    const contents = format === "csv" ? gamesCsv(shown, words) : gamesJson(shown, words);
    const name =
      riotId(store.history.summary(viewer, puuid, platformId)?.name ?? null) || puuid.slice(0, 8);
    try {
      const file = await store.backend.call("save_export", {
        stem: exportStem(t("history.exportPrefix"), name, new Date()),
        format,
        contents,
      });
      toast(t("history.exported", { name: file }), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    }
  };

  // Kept for the next visit, the page and the filter with it.
  useEffect(() => {
    if (loaded.games.length === 0 && loaded.more) return;
    store.history.putList(
      viewer,
      puuid,
      {
        games: loaded.games,
        next: loaded.next,
        more: loaded.more,
        source: loaded.source,
        page,
        filter,
      },
      platformId,
    );
  }, [
    loaded.games,
    loaded.next,
    loaded.more,
    loaded.source,
    page,
    filter,
    puuid,
    viewer,
    store,
    platformId,
  ]);

  // One request at a time. A list drawn from the cache asks for the newest games first: the same
  // ones keep it as it was, new ones start it over from them (the older games have moved down).
  // Then it reads every available page in the background, unless `paused`. This establishes the
  // total, makes the last page addressable and gives the champion view one complete, stable data
  // source.
  useEffect(() => {
    if (!connected || loaded.error || inFlight.current) return;
    if (!(loaded.stale || (loaded.more && !paused))) return;
    inFlight.current = true;
    const revalidating = loaded.stale;
    const begin = revalidating ? 0 : loaded.next;
    const before = loaded.games;
    setLoaded((previous) => ({ ...previous, loading: true }));
    store.backend
      .call("get_match_history", { puuid, begin, count: CHUNK, ...regionArgs(platformId) })
      .then((result) => {
        if (!mounted.current) return;
        const short = result.hasMore && result.next - begin < CHUNK;
        if (revalidating) {
          const same = result.games.every((game, index) => before[index]?.gameId === game.gameId);
          if (same) {
            setLoaded((previous) => ({ ...previous, loading: false, stale: false }));
            return;
          }
          setLoaded({
            games: result.games,
            next: result.next,
            more: result.hasMore,
            loading: false,
            error: null,
            source: result.source,
            stale: false,
            short,
          });
          setPage(1);
          setSelected(null);
          return;
        }
        setLoaded((previous) => {
          const seen = new Set(previous.games.map((game) => game.gameId));
          return {
            games: [...previous.games, ...result.games.filter((game) => !seen.has(game.gameId))],
            next: result.next,
            more: result.hasMore,
            loading: false,
            error: null,
            source: result.source,
            stale: false,
            short,
          };
        });
      })
      .catch((error: unknown) => {
        if (mounted.current) setLoaded((previous) => ({ ...previous, loading: false, error }));
      })
      .finally(() => {
        inFlight.current = false;
      });
  }, [
    connected,
    loaded.error,
    loaded.next,
    loaded.stale,
    loaded.games,
    loaded.more,
    paused,
    puuid,
    platformId,
    store,
  ]);

  const turnTo = (next: number) => {
    setPage(next);
    setSelected(null);
    top.current?.scrollIntoView?.({ block: "nearest" });
  };
  const setHideCustom = (next: boolean) => {
    turnTo(1);
    store
      .updateSettings((settings) => ({
        ...settings,
        history: { ...settings.history, hideCustomGames: next },
      }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  };
  const retry = () => setLoaded((previous) => ({ ...previous, error: null }));
  const filters: { value: Filter; label: string }[] = [
    { value: "all", label: t("history.all") },
    { value: "ranked", label: t("history.ranked") },
    { value: "normal", label: t("history.normal") },
    { value: "aram", label: t("history.aram") },
    { value: "other", label: t("history.other") },
  ];
  const from = (current - 1) * perPage + 1;
  const range =
    visible.length > 0
      ? t("history.range", { from, to: from + visible.length - 1 }) +
        (loaded.more
          ? ` · ${t(paused ? "history.paused" : "history.loadingAll", { n: loaded.games.length })}`
          : ` · ${t("history.total", { n: shown.length })}`)
      : "";

  return (
    <div className="flex flex-col gap-2">
      <div ref={top} className="flex scroll-mt-2 flex-wrap items-center justify-between gap-3">
        <span className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <Segmented
            options={filters}
            value={filter}
            onChange={(value) => {
              setFilter(value);
              turnTo(1);
            }}
            label={t("history.title")}
            size="sm"
          />
          <span className="flex items-center gap-2 text-[12px] text-fg-muted">
            <Toggle id={toggleId} checked={hideCustom} onChange={setHideCustom} />
            <label htmlFor={toggleId} className="cursor-pointer">
              {t("history.hideCustom")}
            </label>
            {hidden > 0 && (
              <span className="mono text-[11px] text-fg-subtle">
                {t("history.hiddenCustom", { n: hidden })}
              </span>
            )}
          </span>
        </span>
        <span className="flex flex-wrap items-center gap-2">
          <span className="mono text-[11px] text-fg-subtle">
            {view === "games"
              ? range
              : loaded.more
                ? t("history.loadingAll", { n: loaded.games.length })
                : t("history.counted", { n: shown.filter((game) => !game.line.remake).length })}
          </span>
          <Segmented<"games" | "champions">
            size="sm"
            label={t("history.view")}
            value={view}
            options={[
              { value: "games", label: t("history.games") },
              { value: "champions", label: t("history.champions") },
            ]}
            onChange={setView}
          />
          <Button
            size="sm"
            variant="ghost"
            icon={Download}
            disabled={shown.length === 0 || loaded.more}
            onClick={() => void exportGames("csv")}
          >
            {t("history.exportCsv")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            icon={Download}
            disabled={shown.length === 0 || loaded.more}
            onClick={() => void exportGames("json")}
          >
            {t("history.exportJson")}
          </Button>
        </span>
      </div>

      {view === "champions" ? (
        <>
          {loaded.more ? (
            <Card>
              {loaded.error ? (
                <ErrorNote
                  {...historyFailure(loaded.error, t)}
                  retryLabel={t("common.retry")}
                  onRetry={retry}
                />
              ) : connected ? (
                <div className="flex items-center justify-center gap-2 py-3 text-[12px] text-fg-muted">
                  <Spinner size={16} label={t("common.loading")} />
                  {t("history.loadingAll", { n: loaded.games.length })}
                </div>
              ) : (
                <EmptyState compact title={t("history.offlinePartial")} />
              )}
            </Card>
          ) : champions.length > 0 ? (
            <ChampionTable rows={champions} />
          ) : (
            <Card>
              {filling || loaded.loading ? (
                <Skeleton className="h-12 w-full" />
              ) : (
                <EmptyState compact title={t("history.emptyFilter")} />
              )}
            </Card>
          )}
        </>
      ) : null}
      {view === "games" && (
        <>
          {visible.length === 0 ? (
            <Card>
              {/* A page that failed stops the filling until a retry: its note shows meanwhile. */}
              {(filling && !loaded.error) || loaded.loading ? (
                <div className="flex flex-col gap-2" aria-busy>
                  {[0, 1, 2].map((key) => (
                    <Skeleton key={key} className="h-12 w-full" />
                  ))}
                </div>
              ) : loaded.error ? (
                <ErrorNote
                  {...historyFailure(loaded.error, t)}
                  retryLabel={t("common.retry")}
                  onRetry={retry}
                />
              ) : hidden > 0 && shown.length === 0 ? (
                <EmptyState
                  compact
                  title={t("history.emptyHidden")}
                  actions={
                    <Button size="sm" onClick={() => setHideCustom(false)}>
                      {t("history.showCustom")}
                    </Button>
                  }
                />
              ) : (
                <EmptyState
                  compact
                  title={loaded.games.length > 0 ? t("history.emptyFilter") : t("history.empty")}
                />
              )}
            </Card>
          ) : (
            <ul className="flex flex-col gap-1.5">
              {visible.map((game) => (
                <li key={game.gameId} className="flex flex-col gap-1.5">
                  <MatchRow
                    game={game}
                    selected={selected === game.gameId}
                    onSelect={() =>
                      setSelected((open) => (open === game.gameId ? null : game.gameId))
                    }
                  />
                  {selected === game.gameId && (
                    <Detail
                      gameId={game.gameId}
                      puuid={puuid}
                      viewer={viewer}
                      connected={connected}
                      platformId={platformId}
                      onPlayer={(other) =>
                        navigate({ page: "history", puuid: other, ...regionArgs(platformId) })
                      }
                    />
                  )}
                </li>
              ))}
            </ul>
          )}

          {visible.length > 0 && (filling || (loaded.stale && Boolean(loaded.error))) && (
            <div className="flex justify-center py-1">
              {loaded.error ? (
                <ErrorNote
                  {...historyFailure(
                    loaded.error,
                    t,
                    loaded.stale ? t("history.refreshFailed") : t("common.loadFailed"),
                  )}
                  retryLabel={t("common.retry")}
                  onRetry={retry}
                />
              ) : (
                <Spinner size={16} label={t("common.loading")} className="text-fg-subtle" />
              )}
            </div>
          )}

          {loaded.games.length > 0 && (
            <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
              <HistoryPager page={current} pages={known} more={loaded.more} onPage={turnTo} />
              <span className="flex items-center gap-2">
                <span className="text-[11.5px] text-fg-subtle">{t("history.pageSize")}</span>
                <Segmented
                  options={PAGE_SIZES.map((value) => ({ value, label: value }))}
                  value={size}
                  onChange={(next) => {
                    setSize(next);
                    savePageSize(next);
                    // The first game on screen stays on screen.
                    turnTo(Math.floor(((current - 1) * perPage) / Number(next)) + 1);
                  }}
                  label={t("history.pageSize")}
                  size="sm"
                />
              </span>
            </div>
          )}
          {loaded.source === "client" && (
            <p className="text-[11.5px] text-fg-subtle">{t("history.clientOnly")}</p>
          )}
        </>
      )}
    </div>
  );
}

function Detail({
  gameId,
  puuid,
  viewer,
  connected,
  platformId,
  onPlayer,
}: {
  gameId: number;
  puuid: string;
  viewer: string;
  connected: boolean;
  platformId?: string;
  onPlayer: (puuid: string) => void;
}) {
  const t = useT();
  const store = useStore();
  const titles = useSettings().general.titles;
  // A finished game never changes: once read, it opens from the cache.
  const detail = useCached(
    store.history.detail(viewer, gameId, platformId),
    () => store.backend.call("get_match_detail", { gameId, ...regionArgs(platformId) }),
    (value) => store.history.putDetail(viewer, value, platformId),
    { refresh: false, deps: [gameId, platformId], enabled: connected },
  );
  return (
    <Card className="ml-1" aria-label={t("history.detail")}>
      {detail.data ? (
        <MatchDetailView
          detail={detail.data}
          highlight={puuid}
          titles={titles}
          onPlayer={(other) => other !== puuid && onPlayer(other)}
        />
      ) : !connected ? (
        <EmptyState compact title={t("history.offlinePartial")} />
      ) : detail.loading ? (
        <div className="flex flex-col gap-2">
          {[0, 1, 2, 3].map((key) => (
            <Skeleton key={key} className="h-8 w-full" />
          ))}
        </div>
      ) : (
        <ErrorNote
          title={t("common.loadFailed")}
          detail={errorMessage(detail.error)}
          retryLabel={t("common.retry")}
          onRetry={detail.reload}
        />
      )}
    </Card>
  );
}

export function HistoryPage({
  puuid: requested,
  platformId,
  regions: searched,
  riotId,
}: {
  puuid?: string;
  platformId?: string;
  regions?: string[];
  riotId?: string;
}) {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const me = useLive((snapshot) => snapshot.me?.puuid);
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const current = useLive((snapshot) =>
    snapshot.connection.status === "connected" ? snapshot.connection.platformId : "",
  );
  // History: what is drawn belongs to the signed-in account; another one starts the page over.
  const viewer = me ?? store.history.viewer ?? "";
  const puuid = requested ?? me ?? (viewer || undefined);
  const cached = Boolean(
    puuid &&
    viewer &&
    (store.history.summary(viewer, puuid, platformId) ||
      store.history.list(viewer, puuid, platformId)),
  );
  // Found on the connected region: whether the account plays on others too is asked afterwards,
  // and its answer is taken only while the page still shows the player it was asked for.
  const [elsewhere, setElsewhere] = useState<{ key: string; regions: string[] } | null>(null);
  const asking = `${puuid ?? ""}\n${riotId ?? ""}`;
  useEffect(() => {
    if (!connected || !puuid || !riotId || platformId || searched || !current) return;
    let live = true;
    store.backend.call("get_player_regions", { riotId }).then(
      (others) => {
        if (live && others.length > 0) setElsewhere({ key: asking, regions: [current, ...others] });
      },
      () => {},
    );
    return () => {
      live = false;
    };
  }, [store, connected, puuid, riotId, platformId, searched, current, asking]);
  const regions = searched ?? (elsewhere?.key === asking ? elsewhere.regions : undefined);
  return (
    <PageBody className="flex flex-col gap-3">
      <SearchBar
        viewer={viewer}
        connected={connected}
        onFound={(found) =>
          navigate({
            page: "history",
            puuid: found.puuid,
            ...regionArgs(found.platformId),
            ...(found.regions ? { regions: found.regions } : {}),
            ...(found.riotId ? { riotId: found.riotId } : {}),
          })
        }
      />
      {!connected && <p className="text-[12px] text-fg-muted">{t("history.offline")}</p>}
      {puuid && (connected || cached) ? (
        <>
          <PlayerHeader
            key={`header:${viewer}:${platformId ?? ""}:${puuid}`}
            puuid={puuid}
            viewer={viewer}
            connected={connected}
            platformId={platformId}
            current={current}
            regions={regions}
            isMe={!platformId && (puuid === me || (!connected && puuid === viewer))}
            onMine={() => navigate({ page: "history" })}
            onRegion={(region) =>
              navigate({
                page: "history",
                puuid,
                ...(region.toUpperCase() === current.toUpperCase() ? {} : { platformId: region }),
                ...(regions ? { regions } : {}),
                ...(riotId ? { riotId } : {}),
              })
            }
          />
          <GameList
            key={`list:${viewer}:${platformId ?? ""}:${puuid}`}
            puuid={puuid}
            viewer={viewer}
            connected={connected}
            platformId={platformId}
          />
        </>
      ) : !connected ? (
        <Card>
          <EmptyState compact title={t("history.offlineEmpty")} />
        </Card>
      ) : null}
    </PageBody>
  );
}
