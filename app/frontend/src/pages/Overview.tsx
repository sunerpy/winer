import { averageLine, duration, riotId, type FriendView, type Settings } from "@winer/shared";
import { ArrowRight, Eye, Swords } from "lucide-react";
import { useMemo } from "react";

import { GroupBadge, groupStripe } from "../game/groups";
import { MatchRow } from "../game/MatchRow";
import { ChampionIcon, ProfileIcon } from "../game/icons";
import { FormLine, KdaValue, RankBadge, ResultStrip, StreakBadge, WinRate } from "../game/stats";
import { errorMessage } from "../lib/backend";
import { cx } from "../lib/cx";
import { useLanguage, useT } from "../lib/i18n";
import { MODES, MODE_LABEL, type ScopedRule } from "../lib/modes";
import { useCatalog, useLive, useNotices, useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { useNow } from "../lib/useNow";
import { useShell } from "../shell/navigation";
import { noticeText } from "../shell/notices";
import { Button, Card, EmptyState, ErrorNote, Panel, Skeleton, Toggle, toast } from "../ui";
import { ConnectionBanner, PageBody } from "./common";

function MeCard() {
  const t = useT();
  const me = useLive((snapshot) => snapshot.me);
  if (!me) {
    return (
      <Panel eyebrow={t("overview.me")} className="col-span-12 @[900px]:col-span-5">
        <div className="flex items-center gap-4">
          <Skeleton className="size-14 rounded-full!" />
          <div className="flex flex-col gap-2">
            <Skeleton className="h-4 w-36" />
            <Skeleton className="h-3 w-20" />
          </div>
        </div>
      </Panel>
    );
  }
  return (
    <Panel eyebrow={t("overview.me")} className="col-span-12 @[900px]:col-span-5">
      <div className="flex items-center gap-4">
        <ProfileIcon id={me.iconId} size={56} />
        <div className="min-w-0">
          <p className="truncate text-[17px] font-semibold text-fg">{riotId(me.name)}</p>
          <p className="mono text-[12px] text-fg-subtle">
            {t("common.level", { level: me.level })}
          </p>
        </div>
      </div>
      <dl className="mt-4 grid grid-cols-2 gap-3">
        {(
          [
            ["common.solo", me.ranked.solo],
            ["common.flex", me.ranked.flex],
          ] as const
        ).map(([label, rank]) => (
          <div key={label} className="rounded-6 bg-inset px-3 py-2.5 hairline">
            <dt className="text-[11px] text-fg-subtle">{t(label)}</dt>
            <dd className="mt-1 flex flex-col gap-1">
              <RankBadge rank={rank} />
              {rank && (
                <span className="text-[11.5px] text-fg-muted">
                  <span className="mono">
                    {rank.wins}
                    {t("common.win")} {rank.losses}
                    {t("common.loss")}
                  </span>{" "}
                  · <WinRate wins={rank.wins} games={rank.wins + rank.losses} />
                </span>
              )}
            </dd>
          </div>
        ))}
      </dl>
    </Panel>
  );
}

function FormCard() {
  const t = useT();
  const store = useStore();
  const puuid = useLive((snapshot) => snapshot.me?.puuid);
  const summary = useAsync(
    () => store.backend.call("get_player_summary", { puuid: puuid ?? "" }),
    [puuid],
    Boolean(puuid),
  );
  const form = summary.data?.recent;

  return (
    <Panel
      eyebrow={t("overview.form")}
      title={form && form.games > 0 ? t("common.recent", { n: form.games }) : undefined}
      right={form && <StreakBadge streak={form.streak} />}
      className="col-span-12 @[900px]:col-span-7"
    >
      {!form ? (
        summary.loading || !puuid ? (
          <div className="flex flex-col gap-3">
            <Skeleton className="h-8 w-40" />
            <Skeleton className="h-4 w-64" />
            <Skeleton className="h-8 w-full" />
          </div>
        ) : summary.error ? (
          <ErrorNote
            title={t("common.loadFailed")}
            detail={errorMessage(summary.error)}
            retryLabel={t("common.retry")}
            onRetry={summary.reload}
          />
        ) : (
          <EmptyState compact title={t("overview.formEmpty")} />
        )
      ) : form.games === 0 ? (
        <EmptyState compact title={t("overview.formEmpty")} />
      ) : (
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-end gap-x-8 gap-y-3">
            <div>
              <p className="text-[11px] text-fg-subtle">{t("common.winRate")}</p>
              <WinRate wins={form.wins} games={form.games} className="text-[28px] leading-8" />
            </div>
            <div>
              <p className="text-[11px] text-fg-subtle">KDA</p>
              <p className="flex items-baseline gap-2">
                <KdaValue
                  kills={form.kills}
                  deaths={form.deaths}
                  assists={form.assists}
                  className="text-[20px] leading-8"
                />
                <span className="mono text-[12px] text-fg-muted">{averageLine(form)}</span>
              </p>
            </div>
            <div className="ml-auto">
              <FormLine form={form} />
            </div>
          </div>
          <ResultStrip matches={form.matches} limit={20} className="[&>span]:h-5 [&>span]:w-2" />
          <div>
            <p className="mb-2 text-[11px] text-fg-subtle">{t("overview.mostPlayed")}</p>
            <ul className="flex flex-wrap gap-3">
              {form.champions.map((champion) => (
                <li
                  key={champion.championId}
                  className="flex items-center gap-2 rounded-6 bg-inset py-1 pr-3 pl-1 hairline"
                >
                  <ChampionIcon id={champion.championId} size={28} />
                  <span className="flex flex-col leading-4">
                    <span className="mono text-[12px] text-fg">
                      {t("common.games", { n: champion.games })}
                    </span>
                    <WinRate wins={champion.wins} games={champion.games} className="text-[11px]" />
                  </span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}
    </Panel>
  );
}

function AutomationCard() {
  const t = useT();
  const store = useStore();
  const automation = useSettings().automation;
  const { navigate } = useShell();
  const save = (change: (automation: Settings["automation"]) => Settings["automation"]) =>
    void store
      .updateSettings((settings) => ({ ...settings, automation: change(settings.automation) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  /** Where a switch acts, in a few words: every mode, the modes picked, or none. */
  const modes = (rule: ScopedRule) => {
    const picked = automation.scopes[rule];
    if (picked.length === 0) return t("auto.scopeNoneShort");
    if (picked.length === MODES.length) return t("auto.scopeAll");
    return picked.map((mode) => t(MODE_LABEL[mode])).join(" · ");
  };
  const rows: {
    rule: ScopedRule;
    label: string;
    readout?: string;
    checked: boolean;
    onChange: (on: boolean) => void;
  }[] = [
    {
      rule: "accept",
      label: t("auto.accept"),
      readout: automation.accept.enabled
        ? t("common.seconds", { n: (automation.accept.delayMs / 1000).toFixed(1) })
        : undefined,
      checked: automation.accept.enabled,
      onChange: (enabled: boolean) =>
        save((value) => ({ ...value, accept: { ...value.accept, enabled } })),
    },
    {
      rule: "pick",
      label: t("auto.pick"),
      readout: automation.pick.enabled
        ? automation.pick.lockIn
          ? t("auto.lockInLock")
          : t("auto.lockInHover")
        : undefined,
      checked: automation.pick.enabled,
      onChange: (enabled: boolean) =>
        save((value) => ({ ...value, pick: { ...value.pick, enabled } })),
    },
    {
      rule: "ban",
      label: t("auto.ban"),
      checked: automation.ban.enabled,
      onChange: (enabled: boolean) =>
        save((value) => ({ ...value, ban: { ...value.ban, enabled } })),
    },
    {
      rule: "playAgain",
      label: t("auto.playAgain"),
      checked: automation.playAgain,
      onChange: (playAgain: boolean) => save((value) => ({ ...value, playAgain })),
    },
    {
      rule: "callout",
      label: t("auto.calloutAuto"),
      readout: automation.callout.auto
        ? automation.callout.audience === "team"
          ? t("auto.audience.team")
          : t("auto.audience.me")
        : undefined,
      checked: automation.callout.auto,
      onChange: (auto: boolean) =>
        save((value) => ({ ...value, callout: { ...value.callout, auto } })),
    },
    {
      rule: "bench",
      label: t("auto.bench"),
      readout: automation.bench.enabled
        ? t("common.count", { n: automation.bench.champions.length })
        : undefined,
      checked: automation.bench.enabled,
      onChange: (enabled: boolean) =>
        save((value) => ({ ...value, bench: { ...value.bench, enabled } })),
    },
  ];

  return (
    <Panel
      eyebrow={t("overview.automation")}
      right={
        <Button variant="link" size="sm" onClick={() => navigate({ page: "automation" })}>
          {t("overview.configure")}
        </Button>
      }
      className="col-span-12 @[900px]:col-span-5"
    >
      <ul className="flex flex-col">
        {rows.map((row) => (
          <li
            key={row.rule}
            className="flex min-h-11 items-center justify-between gap-3 border-b border-border py-1 last:border-b-0"
          >
            <span className="flex min-w-0 flex-col">
              <span className="text-[13px] text-fg">{row.label}</span>
              <span className="truncate text-[11px] text-fg-subtle">{modes(row.rule)}</span>
            </span>
            <span className="flex items-center gap-3">
              {row.readout && (
                <span className="mono text-[11px] text-fg-subtle">{row.readout}</span>
              )}
              <Toggle checked={row.checked} onChange={row.onChange} label={row.label} />
            </span>
          </li>
        ))}
      </ul>
    </Panel>
  );
}

function ActivityCard() {
  const t = useT();
  const catalog = useCatalog();
  const language = useLanguage();
  const notices = useNotices();
  return (
    <Panel
      eyebrow={t("overview.activity")}
      className="col-span-12 @[900px]:col-span-7"
      bodyClassName="max-h-[180px] overflow-y-auto"
    >
      {notices.length === 0 ? (
        <EmptyState compact title={t("overview.activityEmpty")} />
      ) : (
        <ol className="flex flex-col">
          {notices.map((notice) => (
            <li
              key={notice.id}
              className="flex h-8 items-center gap-3 border-b border-border text-[12.5px] last:border-b-0"
            >
              <span className="mono w-16 shrink-0 text-[11px] text-fg-subtle">
                {new Date(notice.at).toLocaleTimeString(language, { hour12: false })}
              </span>
              <span className={notice.kind.kind === "failed" ? "text-danger" : "text-fg"}>
                {noticeText(notice.kind, t, catalog)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}

/** In champ select or in a game: what the friends panel lists. */
function playing(friend: FriendView): boolean {
  return friend.status.state === "inGame" || friend.status.state === "champSelect";
}

/** One friend at play: the mode, how long it has been going (ticking here, from the start the
 *  presence gives), and the colour and number shared with the friends in the same game. */
function FriendRow({ friend, now }: { friend: FriendView; now: number }) {
  const t = useT();
  const { navigate } = useShell();
  const status = friend.status;
  const name = riotId(friend.name) || t("common.hidden");
  const [state, mode, since] =
    status.state === "inGame"
      ? [t("social.inGame"), status.mode, status.startedAt]
      : status.state === "champSelect"
        ? [t("social.champSelect"), status.mode, status.since]
        : ["", "", 0];
  const elapsed = since > 0 ? duration((now - since) / 1000) : null;
  return (
    <li
      data-group={friend.group ?? undefined}
      className={cx(
        "min-w-0 rounded-6 bg-inset hairline",
        friend.group !== null && groupStripe(friend.group),
      )}
    >
      <button
        type="button"
        aria-label={t("live.openHistory", { name })}
        onClick={() => navigate({ page: "history", puuid: friend.puuid })}
        className="flex w-full min-w-0 items-center gap-2.5 rounded-6 py-2 pr-2.5 pl-3 text-left hover-wash"
      >
        <ProfileIcon id={friend.iconId} size={28} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-[13px] font-medium text-fg">{name}</span>
          <span className="truncate text-[11.5px] text-fg-subtle">
            {mode ? `${mode} · ${state}` : state}
          </span>
        </span>
        {friend.group !== null && (
          <GroupBadge
            group={friend.group}
            label={String(friend.group)}
            title={t("social.together", { n: friend.group })}
          />
        )}
        {status.state === "inGame" && status.observable && (
          <span title={t("social.observable")} className="shrink-0 text-fg-subtle">
            <Eye size={13} strokeWidth={2} aria-hidden />
            <span className="sr-only">{t("social.observable")}</span>
          </span>
        )}
        {elapsed && (
          <span
            title={t("social.elapsed", { time: elapsed })}
            className="mono w-12 shrink-0 text-right text-[12px] text-fg-muted"
          >
            {elapsed}
          </span>
        )}
      </button>
    </li>
  );
}

function FriendsCard() {
  const t = useT();
  const friends = useLive((snapshot) => snapshot.friends);
  const shown = useMemo(() => friends?.friends.filter(playing) ?? [], [friends]);
  const now = useNow(1000, shown.length > 0);
  return (
    <Panel
      eyebrow={t("social.friends")}
      title={shown.length > 0 ? t("social.friendsPlaying", { n: shown.length }) : undefined}
      className="col-span-12"
    >
      {friends === null ? (
        <div aria-busy className="grid grid-cols-2 gap-1.5">
          <span className="sr-only">{t("social.friendsLoading")}</span>
          {[0, 1].map((key) => (
            <Skeleton key={key} className="h-12 w-full" />
          ))}
        </div>
      ) : shown.length === 0 ? (
        <EmptyState compact title={t("social.friendsEmpty")} />
      ) : (
        <div className="@container">
          <ul className="grid grid-cols-1 gap-1.5 @[640px]:grid-cols-2 @[960px]:grid-cols-3">
            {shown.map((friend) => (
              <FriendRow key={friend.puuid} friend={friend} now={now} />
            ))}
          </ul>
          {shown.some((friend) => friend.group !== null) && (
            <p className="mt-2 text-[11.5px] text-fg-subtle">{t("social.togetherHint")}</p>
          )}
        </div>
      )}
    </Panel>
  );
}

function RecentMatches() {
  const t = useT();
  const store = useStore();
  const { navigate } = useShell();
  const puuid = useLive((snapshot) => snapshot.me?.puuid);
  const page = useAsync(
    () => store.backend.call("get_match_history", { puuid: puuid ?? "", begin: 0, count: 5 }),
    [puuid],
    Boolean(puuid),
  );
  const now = Date.now();
  return (
    <Panel
      eyebrow={t("overview.recentMatches")}
      right={
        <Button variant="link" size="sm" onClick={() => navigate({ page: "history" })}>
          {t("overview.allMatches")}
          <ArrowRight size={13} strokeWidth={2} aria-hidden />
        </Button>
      }
      className="col-span-12"
    >
      {page.data && page.data.games.length > 0 ? (
        <div className="flex flex-col gap-1.5">
          {page.data.games.map((game) => (
            <MatchRow
              key={game.gameId}
              game={game}
              dense
              now={now}
              onSelect={() => navigate({ page: "history" })}
            />
          ))}
        </div>
      ) : page.loading ? (
        <div className="flex flex-col gap-1.5">
          {[0, 1, 2].map((key) => (
            <Skeleton key={key} className="h-14 w-full rounded-10!" />
          ))}
        </div>
      ) : page.error ? (
        <ErrorNote
          title={t("common.loadFailed")}
          detail={errorMessage(page.error)}
          retryLabel={t("common.retry")}
          onRetry={page.reload}
        />
      ) : (
        <EmptyState compact title={t("history.empty")} />
      )}
    </Panel>
  );
}

export function OverviewPage() {
  const t = useT();
  const { navigate } = useShell();
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const live = useLive((snapshot) => snapshot.champSelect !== null || snapshot.game !== null);
  return (
    <PageBody className="flex flex-col gap-3">
      {/* The title and status bars already say "connected"; the banner speaks only when there is
          something to do: find the client, or jump into the game in progress. */}
      {(!connected || live) && (
        <ConnectionBanner
          actions={
            live ? (
              <Button
                variant="accent"
                size="sm"
                icon={Swords}
                onClick={() => navigate({ page: "live" })}
              >
                {t("overview.open")}
              </Button>
            ) : undefined
          }
        />
      )}
      {connected ? (
        <div className="grid grid-cols-12 gap-3">
          <MeCard />
          <FormCard />
          <AutomationCard />
          <ActivityCard />
          <FriendsCard />
          <RecentMatches />
        </div>
      ) : (
        <div className="grid grid-cols-12 gap-3">
          <AutomationCard />
          <ActivityCard />
          <Card className="col-span-12">
            <EmptyState compact title={t("connection.searchingHint")} />
          </Card>
        </div>
      )}
    </PageBody>
  );
}
