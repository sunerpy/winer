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
import { type ExportWords, championRows, exportStem, gamesCsv, gamesJson } from "./history/records";

const PAGE_SIZES = ["10", "15", "25", "50"] as const;
type PageSize = (typeof PAGE_SIZES)[number];
const PAGE_SIZE_KEY = "winer.history.pageSize";
/** Games asked for at a time, the most one request returns. */
const CHUNK = 50;

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

function PlayerHeader({
  puuid,
  viewer,
  connected,
  isMe,
  onMine,
}: {
  puuid: string;
  /** The signed-in account, whose cache this is. */
  viewer: string;
  connected: boolean;
  isMe: boolean;
  onMine: () => void;
}) {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  // Drawn at once on a revisit, then read again: a game may have ended since.
  const summary = useCached(
    store.history.summary(viewer, puuid),
    () => store.backend.call("get_player_summary", { puuid }),
    (value) => store.history.putSummary(viewer, value),
    { refresh: connected, deps: [puuid], enabled: connected },
  );
  // The tier and its words follow the rating settings; asked once the record is in, so the core
  // reads the player once.
  const { tiers, customTiers } = settings.automation.callout;
  const { language, titles } = settings.general;
  const standing = useAsync(
    () => store.backend.call("get_player_standing", { puuid }),
    [puuid, tiers, customTiers.join("\n"), language, titles],
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
            <span className="flex flex-col gap-1">
              <span className="text-[11px] text-fg-subtle">{t("common.solo")}</span>
              <RankBadge rank={player.ranked.solo} />
            </span>
            <span className="flex flex-col gap-1">
              <span className="text-[11px] text-fg-subtle">{t("common.flex")}</span>
              <RankBadge rank={player.ranked.flex} />
            </span>
            {player.recent.games > 0 && (
              <span className="flex flex-col gap-1">
                <span className="text-[11px] text-fg-subtle">
                  <FormLabel form={player.recent} scope={standing.data?.scope ?? null} />
                </span>
                <span className="flex items-center gap-2">
                  <FormLine form={player.recent} />
                  <StreakBadge streak={player.recent.streak} />
                </span>
                <ResultStrip matches={player.recent.matches} limit={20} />
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
            title={t("common.loadFailed")}
            detail={errorMessage(summary.error)}
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
  onFound: (puuid: string) => void;
}) {
  const t = useT();
  const store = useStore();
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [invalid, setInvalid] = useState(false);

  const submit = async () => {
    if (!query.trim()) return;
    setBusy(true);
    setInvalid(false);
    try {
      if (!connected) {
        const cached = store.history.find(viewer, query);
        if (!cached) throw new Error(t("history.offlineMissing"));
        onFound(cached);
        return;
      }
      const player = await store.backend.call("find_player", { riotId: query });
      store.history.remember(viewer, player);
      onFound(player.puuid);
    } catch (error) {
      const cached = store.history.find(viewer, query);
      if (cached) {
        onFound(cached);
        return;
      }
      setInvalid(true);
      toast(
        !connected
          ? t("history.offlineMissing")
          : errorCode(error) === "notFound"
            ? t("history.notFound")
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
      className="flex items-center gap-2"
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
}

const NOTHING_YET: Loaded = {
  games: [],
  next: 0,
  more: true,
  loading: false,
  error: null,
  source: null,
  stale: false,
};

function GameList({
  puuid,
  viewer,
  connected,
}: {
  puuid: string;
  viewer: string;
  connected: boolean;
}) {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const hideCustom = useSettings().history.hideCustomGames;
  // Where the list was left on the last visit, if it was this account's.
  const [kept] = useState(() => store.history.list(viewer, puuid));
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
  const filling = connected && loaded.more;
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
    const name = riotId(store.history.summary(viewer, puuid)?.name ?? null) || puuid.slice(0, 8);
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
    store.history.putList(viewer, puuid, {
      games: loaded.games,
      next: loaded.next,
      more: loaded.more,
      source: loaded.source,
      page,
      filter,
    });
  }, [loaded.games, loaded.next, loaded.more, loaded.source, page, filter, puuid, viewer, store]);

  // One request at a time. A list drawn from the cache asks for the newest games first: the same
  // ones keep it as it was, new ones start it over from them (the older games have moved down).
  // Then it reads every available page in the background. This establishes the total, makes the
  // last page addressable and gives the champion view one complete, stable data source.
  useEffect(() => {
    if (!connected || loaded.error || inFlight.current || !(loaded.stale || loaded.more)) return;
    inFlight.current = true;
    const revalidating = loaded.stale;
    const begin = revalidating ? 0 : loaded.next;
    const before = loaded.games;
    setLoaded((previous) => ({ ...previous, loading: true }));
    store.backend
      .call("get_match_history", { puuid, begin, count: CHUNK })
      .then((result) => {
        if (!mounted.current) return;
        if (revalidating) {
          const same = result.games.every((game, index) => before[index]?.gameId === game.gameId);
          if (same) {
            setLoaded((previous) => ({ ...previous, loading: false, stale: false }));
            return;
          }
          setLoaded({
            games: result.games,
            next: CHUNK,
            more: result.hasMore,
            loading: false,
            error: null,
            source: result.source,
            stale: false,
          });
          setPage(1);
          setSelected(null);
          return;
        }
        setLoaded((previous) => {
          const seen = new Set(previous.games.map((game) => game.gameId));
          return {
            games: [...previous.games, ...result.games.filter((game) => !seen.has(game.gameId))],
            next: begin + CHUNK,
            more: result.hasMore,
            loading: false,
            error: null,
            source: result.source,
            stale: false,
          };
        });
      })
      .catch((error: unknown) => {
        if (mounted.current) setLoaded((previous) => ({ ...previous, loading: false, error }));
      })
      .finally(() => {
        inFlight.current = false;
      });
  }, [connected, loaded.error, loaded.next, loaded.stale, loaded.games, loaded.more, puuid, store]);

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
          ? ` · ${t("history.loadingAll", { n: loaded.games.length })}`
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
                  title={t("common.loadFailed")}
                  detail={errorMessage(loaded.error)}
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
              {filling || loaded.loading ? (
                <div className="flex flex-col gap-2" aria-busy>
                  {[0, 1, 2].map((key) => (
                    <Skeleton key={key} className="h-12 w-full" />
                  ))}
                </div>
              ) : loaded.error ? (
                <ErrorNote
                  title={t("common.loadFailed")}
                  detail={errorMessage(loaded.error)}
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
                      onPlayer={(other) => navigate({ page: "history", puuid: other })}
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
                  title={loaded.stale ? t("history.refreshFailed") : t("common.loadFailed")}
                  detail={errorMessage(loaded.error)}
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
  onPlayer,
}: {
  gameId: number;
  puuid: string;
  viewer: string;
  connected: boolean;
  onPlayer: (puuid: string) => void;
}) {
  const t = useT();
  const store = useStore();
  const titles = useSettings().general.titles;
  // A finished game never changes: once read, it opens from the cache.
  const detail = useCached(
    store.history.detail(viewer, gameId),
    () => store.backend.call("get_match_detail", { gameId }),
    (value) => store.history.putDetail(viewer, value),
    { refresh: false, deps: [gameId], enabled: connected },
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

export function HistoryPage({ puuid: requested }: { puuid?: string }) {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const me = useLive((snapshot) => snapshot.me?.puuid);
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  // History: what is drawn belongs to the signed-in account; another one starts the page over.
  const viewer = me ?? store.history.viewer ?? "";
  const puuid = requested ?? me ?? (viewer || undefined);
  const cached = Boolean(
    puuid && viewer && (store.history.summary(viewer, puuid) || store.history.list(viewer, puuid)),
  );
  return (
    <PageBody className="flex flex-col gap-3">
      <SearchBar
        viewer={viewer}
        connected={connected}
        onFound={(found) => navigate({ page: "history", puuid: found })}
      />
      {!connected && <p className="text-[12px] text-fg-muted">{t("history.offline")}</p>}
      {puuid && (connected || cached) ? (
        <>
          <PlayerHeader
            key={`header:${viewer}:${puuid}`}
            puuid={puuid}
            viewer={viewer}
            connected={connected}
            isMe={puuid === me || (!connected && puuid === viewer)}
            onMine={() => navigate({ page: "history" })}
          />
          <GameList
            key={`list:${viewer}:${puuid}`}
            puuid={puuid}
            viewer={viewer}
            connected={connected}
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
