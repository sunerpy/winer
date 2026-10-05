import { riotId, type HistorySource, type MatchSummary } from "@winer/shared";
import { Search, UserRound } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { MatchDetailView } from "../game/MatchDetailView";
import { MatchRow } from "../game/MatchRow";
import { ProfileIcon } from "../game/icons";
import { FormLine, RankBadge, ResultStrip, StreakBadge } from "../game/stats";
import { errorCode, errorMessage } from "../lib/backend";
import { useT } from "../lib/i18n";
import { useLive, useSettings, useStore } from "../lib/store";
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
  toast,
} from "../ui";
import { ConnectionGate, PageBody } from "./common";

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

type Filter = "all" | "ranked" | "normal" | "aram" | "other";

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
  isMe,
  onMine,
}: {
  puuid: string;
  isMe: boolean;
  onMine: () => void;
}) {
  const t = useT();
  const store = useStore();
  const summary = useAsync(() => store.backend.call("get_player_summary", { puuid }), [puuid]);
  const player = summary.data;
  return (
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
            <span className="flex flex-col gap-1.5">
              <span className="flex items-center gap-2">
                <FormLine form={player.recent} />
                <StreakBadge streak={player.recent.streak} />
              </span>
              <ResultStrip matches={player.recent.matches} limit={20} />
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
  );
}

function SearchBar({ onFound }: { onFound: (puuid: string) => void }) {
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
      const player = await store.backend.call("find_player", { riotId: query });
      onFound(player.puuid);
    } catch (error) {
      setInvalid(true);
      toast(
        errorCode(error) === "notFound" ? t("history.notFound") : errorMessage(error),
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
}

function GameList({ puuid }: { puuid: string }) {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const [loaded, setLoaded] = useState<Loaded>({
    games: [],
    next: 0,
    more: true,
    loading: false,
    error: null,
    source: null,
  });
  const [filter, setFilter] = useState<Filter>("all");
  const [size, setSize] = useState<PageSize>(readPageSize);
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<number | null>(null);
  const top = useRef<HTMLDivElement>(null);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const perPage = Number(size);
  const shown = useMemo(
    () => loaded.games.filter((game) => matches(filter, game)),
    [loaded.games, filter],
  );
  const known = Math.ceil(shown.length / perPage);
  // Past the last page there is nothing to show; the last one stands in.
  const current = loaded.more ? page : Math.min(page, Math.max(1, known));
  const visible = shown.slice((current - 1) * perPage, current * perPage);
  const filling = shown.length < current * perPage && loaded.more;

  // Reads on while the page asked for is not full and the server may have more: a filter that few
  // games match fills its page from as many requests as it takes.
  useEffect(() => {
    if (!filling || loaded.error || inFlight.current) return;
    inFlight.current = true;
    const begin = loaded.next;
    setLoaded((previous) => ({ ...previous, loading: true }));
    store.backend
      .call("get_match_history", { puuid, begin, count: CHUNK })
      .then((result) => {
        if (!mounted.current) return;
        setLoaded((previous) => {
          const seen = new Set(previous.games.map((game) => game.gameId));
          return {
            games: [...previous.games, ...result.games.filter((game) => !seen.has(game.gameId))],
            next: begin + CHUNK,
            more: result.hasMore,
            loading: false,
            error: null,
            source: result.source,
          };
        });
      })
      .catch((error: unknown) => {
        if (mounted.current) setLoaded((previous) => ({ ...previous, loading: false, error }));
      })
      .finally(() => {
        inFlight.current = false;
      });
  }, [filling, loaded.error, loaded.next, puuid, store]);

  const turnTo = (next: number) => {
    setPage(next);
    setSelected(null);
    top.current?.scrollIntoView?.({ block: "nearest" });
  };
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
        (loaded.more ? "" : ` · ${t("history.total", { n: shown.length })}`)
      : "";

  return (
    <div className="flex flex-col gap-2">
      <div ref={top} className="flex scroll-mt-2 items-center justify-between gap-3">
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
        <span className="mono text-[11px] text-fg-subtle">{range}</span>
      </div>

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
              onRetry={() => setLoaded((previous) => ({ ...previous, error: null }))}
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
                onSelect={() => setSelected((open) => (open === game.gameId ? null : game.gameId))}
              />
              {selected === game.gameId && (
                <Detail
                  gameId={game.gameId}
                  puuid={puuid}
                  onPlayer={(other) => navigate({ page: "history", puuid: other })}
                />
              )}
            </li>
          ))}
        </ul>
      )}

      {visible.length > 0 && filling && (
        <div className="flex justify-center py-1">
          {loaded.error ? (
            <ErrorNote
              title={t("common.loadFailed")}
              detail={errorMessage(loaded.error)}
              retryLabel={t("common.retry")}
              onRetry={() => setLoaded((previous) => ({ ...previous, error: null }))}
            />
          ) : (
            <Spinner size={16} label={t("common.loading")} className="text-fg-subtle" />
          )}
        </div>
      )}

      {loaded.games.length > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
          <Pager
            page={current}
            known={known}
            more={loaded.more}
            onPage={turnTo}
            labels={{
              nav: t("history.pages"),
              previous: t("history.previous"),
              next: t("history.next"),
              page: (n) => t("history.page", { n }),
            }}
          />
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
    </div>
  );
}

function Detail({
  gameId,
  puuid,
  onPlayer,
}: {
  gameId: number;
  puuid: string;
  onPlayer: (puuid: string) => void;
}) {
  const t = useT();
  const store = useStore();
  const titles = useSettings().general.titles;
  const detail = useAsync(() => store.backend.call("get_match_detail", { gameId }), [gameId]);
  return (
    <Card className="ml-1" aria-label={t("history.detail")}>
      {detail.data ? (
        <MatchDetailView
          detail={detail.data}
          highlight={puuid}
          titles={titles}
          onPlayer={(other) => other !== puuid && onPlayer(other)}
        />
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
  const { navigate } = useShell();
  const me = useLive((snapshot) => snapshot.me?.puuid);
  const puuid = requested ?? me;
  return (
    <PageBody className="flex flex-col gap-3">
      <ConnectionGate offline={t("history.offline")}>
        <SearchBar onFound={(found) => navigate({ page: "history", puuid: found })} />
        {puuid && (
          <>
            <PlayerHeader
              puuid={puuid}
              isMe={puuid === me}
              onMine={() => navigate({ page: "history" })}
            />
            <GameList key={puuid} puuid={puuid} />
          </>
        )}
      </ConnectionGate>
    </PageBody>
  );
}
