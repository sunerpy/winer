import type { Audience, ChampSelectView, GameView, LobbyView, Seat, Side } from "@winer/shared";
import { Ban, Dices, Eye, Hourglass, Megaphone, Star, Swords, Users } from "lucide-react";
import { type ReactNode, useState } from "react";

import { LobbyBoard } from "../game/LobbyBoard";
import { queueName } from "../game/MatchRow";
import { TeamBoard } from "../game/TeamBoard";
import { ChampionIcon } from "../game/icons";
import { errorMessage } from "../lib/backend";
import { GAME_LINE_LIMIT } from "../lib/callout";
import { cx } from "../lib/cx";
import { type MessageKey, useT } from "../lib/i18n";
import { everywhere, modeOf } from "../lib/modes";
import { useCatalog, useHotkeyStatus, useLive, useSettings, useStore } from "../lib/store";
import { useNow } from "../lib/useNow";
import { useShell } from "../shell/navigation";
import { Badge, Button, Card, EmptyState, Lamp, Panel, Segmented, toast } from "../ui";
import { ConnectionGate, PageBody } from "./common";
import { BuildLookup, ChampSelectBuild, GameBuild } from "./live/BuildPanel";
import { Keycaps } from "./settings/HotkeyRow";

const TIMER_PHASES: Record<string, MessageKey> = {
  PLANNING: "live.planning",
  BAN_PICK: "live.banPick",
  FINALIZATION: "live.finalization",
  GAME_STARTING: "live.gameStarting",
};

const SIDE_DOT: Record<Side, string> = { blue: "bg-side-blue", red: "bg-side-red" };

function opposite(side: Side | null): Side | null {
  return side && (side === "blue" ? "red" : "blue");
}

interface TeamTab {
  title: string;
  side: Side | null;
  seats: Seat[];
}

/** The analysis board of one team at a time, the local player's first. */
function Teams({ teams, initial = 0 }: { teams: TeamTab[]; initial?: number }) {
  const t = useT();
  const { navigate } = useShell();
  const [shown, setShown] = useState(initial);
  const team = teams[shown] ?? teams[0];
  if (!team) return null;
  const name = (tab: TeamTab) => (tab.side ? `${tab.title} · ${t(`live.${tab.side}`)}` : tab.title);
  return (
    <Panel
      eyebrow={t("live.board")}
      title={teams.length === 1 ? undefined : t("live.boardHint")}
      right={
        teams.length > 1 ? (
          <Segmented
            size="sm"
            label={t("live.board")}
            value={String(shown)}
            options={teams.map((tab, index) => ({ value: String(index), label: name(tab) }))}
            onChange={(value) => setShown(Number(value))}
          />
        ) : (
          <span className="flex items-center gap-1.5 text-[12px] font-medium text-fg-muted">
            {team.side && (
              <span aria-hidden className={cx("size-2 rounded-full", SIDE_DOT[team.side])} />
            )}
            {name(team)}
          </span>
        )
      }
    >
      <TeamBoard seats={team.seats} onPlayer={(puuid) => navigate({ page: "history", puuid })} />
    </Panel>
  );
}

function BanRow({ label, ids }: { label: string; ids: number[] }) {
  if (ids.length === 0) return null;
  return (
    <span className="flex items-center gap-1.5">
      <span className="text-[11px] text-fg-subtle">{label}</span>
      {ids.map((id) => (
        <ChampionIcon key={id} id={id} size={24} className="grayscale" />
      ))}
    </span>
  );
}

/** ARAM's bench: every champion is a button that swaps at once, cooldown or not. */
function Bench({ view }: { view: ChampSelectView }) {
  const t = useT();
  const store = useStore();
  const catalog = useCatalog();
  const wishlist = useSettings().automation.bench.champions;
  const [busy, setBusy] = useState<number | "reroll" | null>(null);
  const run = async (target: number | "reroll") => {
    setBusy(target);
    try {
      if (target === "reroll") await store.backend.call("reroll");
      else await store.backend.call("bench_swap", { championId: target });
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };
  return (
    <Card padding="sm" className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <span className="flex flex-col gap-0.5">
        <span className="eyebrow">{t("live.bench")}</span>
        <span className="text-[11px] text-fg-subtle">{t("live.benchHint")}</span>
      </span>
      <ul className="flex min-h-9 flex-wrap items-center gap-1.5">
        {view.bench.map((id) => {
          const wished = wishlist.includes(id);
          const label = t("live.swap", {
            champion: catalog?.champions.get(id)?.shortName ?? `#${id}`,
          });
          return (
            <li key={id}>
              <button
                type="button"
                aria-label={wished ? `${label} · ${t("live.wished")}` : label}
                title={wished ? `${label} · ${t("live.wished")}` : label}
                disabled={busy !== null}
                onClick={() => void run(id)}
                className="relative block rounded-6 transition-transform duration-150 hover:-translate-y-px disabled:opacity-60"
              >
                <ChampionIcon id={id} size={36} />
                {wished && (
                  <Star
                    aria-hidden
                    size={13}
                    strokeWidth={2}
                    className="absolute -top-1 -right-1 fill-accent text-accent"
                  />
                )}
              </button>
            </li>
          );
        })}
      </ul>
      {view.rerollsRemaining > 0 && (
        <Button
          size="sm"
          icon={Dices}
          className="ml-auto"
          loading={busy === "reroll"}
          disabled={busy !== null && busy !== "reroll"}
          onClick={() => void run("reroll")}
        >
          {t("live.reroll", { n: view.rerollsRemaining })}
        </Button>
      )}
    </Card>
  );
}

/** The lines exactly as they go out, in an inset block; `empty` says what comes there. */
function CalloutLines({ lines, empty }: { lines: string[]; empty: string }) {
  if (lines.length === 0)
    return (
      <p className="rounded-6 border border-dashed border-border-strong px-3 py-3 text-center text-[12px] text-fg-subtle">
        {empty}
      </p>
    );
  return (
    <ol className="flex flex-col gap-1 rounded-6 bg-inset px-3 py-2 hairline">
      {lines.map((line, index) => (
        <li key={index} className="text-[12.5px] leading-5 break-words text-fg">
          {line}
        </li>
      ))}
    </ol>
  );
}

/** The callout: in champ select the team ranked by recent form, as the chat lines it would send;
 *  in the game (`game`), the enemy to watch and the one to go after beside the team's own lines,
 *  every player by champion, which only the shortcut can type, since the game's chat has no API. */
function Callout({
  lines,
  queueId,
  game = false,
  allies = [],
}: {
  lines: string[];
  queueId: number;
  game?: boolean;
  /** In the game: the team's own lines. */
  allies?: string[];
}) {
  const t = useT();
  const store = useStore();
  const catalog = useCatalog();
  const { callout, scopes } = useSettings().automation;
  const queue = catalog?.queues.get(queueId);
  // Goes out by itself only in the modes it is scoped to, as the core decides it.
  const auto =
    callout.auto &&
    (queue
      ? scopes.callout.includes(modeOf(queue.gameMode, queue.ranked))
      : everywhere("callout", scopes.callout));
  const [busy, setBusy] = useState<Audience | null>(null);
  const send = async (audience: Audience) => {
    setBusy(audience);
    try {
      const sent = await store.backend.call("send_callout", { audience });
      toast(t("live.sent", { n: sent }), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };
  // Callout: in the game, whether the shortcut types there.
  const badge = game ? (
    <Badge tone={callout.inGame ? "accent" : "neutral"}>
      {t(callout.inGame ? "callout.inGameOn" : "callout.inGameOff")}
    </Badge>
  ) : auto ? (
    <Badge tone="accent">{t("live.calloutAuto")}</Badge>
  ) : undefined;
  return (
    <Panel eyebrow={t("live.callout")} right={badge}>
      <p className="mb-2.5 text-[12px] leading-5 text-fg-muted">
        {t(game ? "callout.liveGameHint" : "live.calloutHint")}
      </p>
      {game ? (
        // Callout: in the game both teams, side by side where there is room.
        <div className="@container">
          <div className="grid grid-cols-1 gap-3 @[720px]:grid-cols-2">
            {(
              [
                ["enemies", lines, "callout.liveGameEmpty"],
                ["allies", allies, "callout.liveAlliesEmpty"],
              ] as const
            ).map(([teams, shown, empty]) => (
              <section
                key={teams}
                aria-labelledby={`callout-${teams}`}
                className="flex min-w-0 flex-col gap-1.5"
              >
                <h3 id={`callout-${teams}`} className="text-[12px] font-medium text-fg-muted">
                  {t(`callout.gameTeams.${teams}`)}
                </h3>
                <CalloutLines lines={shown} empty={t(empty)} />
              </section>
            ))}
          </div>
        </div>
      ) : (
        <CalloutLines lines={lines} empty={t("live.calloutEmpty")} />
      )}
      {!game && (
        <div className="mt-3 flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="accent"
            icon={Megaphone}
            disabled={lines.length === 0 || busy !== null}
            loading={busy === "team"}
            onClick={() => void send("team")}
          >
            {t("live.sendTeam")}
          </Button>
          <Button
            size="sm"
            icon={Eye}
            disabled={lines.length === 0 || busy !== null}
            loading={busy === "me"}
            onClick={() => void send("me")}
          >
            {t("live.sendMe")}
          </Button>
        </div>
      )}
      <CalloutShortcut game={game} />
    </Panel>
  );
}

/** Callout: one line naming the shortcut that sends the callout and what it does here, with the
 *  way to the settings that change it. */
function CalloutShortcut({ game }: { game: boolean }) {
  const t = useT();
  const { navigate } = useShell();
  const { hotkey, inGame, gameTeams } = useSettings().automation.callout;
  const status = useHotkeyStatus();
  const refused =
    hotkey !== null &&
    status !== null &&
    !status.suspended &&
    status.callout.shortcut === hotkey &&
    status.callout.error !== null;
  let lamp: "ok" | "off" | "danger" = "off";
  let text: ReactNode;
  if (hotkey === null) {
    text = t(game ? "callout.liveNoHotkeyGame" : "callout.liveNoHotkey");
  } else if (refused) {
    lamp = "danger";
    text = t("callout.liveHotkeyFailed");
  } else if (game && !inGame) {
    text = t("callout.liveInGameOff");
  } else {
    lamp = "ok";
    // Callout: in the game, whose lines one press types, and how many at most.
    text = (
      <>
        {game
          ? t(`callout.liveHotkeyGame.${gameTeams}`, { n: GAME_LINE_LIMIT })
          : t("callout.liveHotkey")}
        <Keycaps combo={hotkey} />
      </>
    );
  }
  return (
    <div className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 border-t border-border pt-2.5 text-[12px] leading-5 text-fg-muted">
      <Lamp tone={lamp} size={6} />
      <span className="inline-flex flex-wrap items-center gap-1.5">{text}</span>
      <Button size="sm" variant="link" onClick={() => navigate({ page: "automation" })}>
        {t("callout.configure")}
      </Button>
    </div>
  );
}

function ChampSelect({ view }: { view: ChampSelectView }) {
  const t = useT();
  const catalog = useCatalog();
  const now = useNow(500, view.timer.endsAt > 0);
  const seconds =
    view.timer.endsAt > 0 ? Math.max(0, Math.ceil((view.timer.endsAt - now) / 1000)) : null;
  const phase = TIMER_PHASES[view.timer.phase];

  return (
    <div className="flex flex-col gap-4">
      <Card className="flex flex-wrap items-center gap-x-6 gap-y-2">
        <span className="flex items-center gap-2.5">
          <Swords size={16} strokeWidth={2} className="text-accent-text" aria-hidden />
          <span className="text-[15px] font-semibold text-fg">
            {queueName(view.queueId, "", catalog, t("phase.ChampSelect"))}
          </span>
          {phase && <Badge tone="accent">{t(phase)}</Badge>}
        </span>
        {seconds !== null && (
          <span className="mono flex items-center gap-1.5 text-[13px] text-fg-muted">
            <Hourglass size={13} strokeWidth={2} aria-hidden />
            {t("live.timer", { s: seconds })}
          </span>
        )}
        {view.myBans.length + view.theirBans.length > 0 && (
          <span className="ml-auto flex flex-wrap items-center gap-4">
            <Ban size={13} strokeWidth={2} aria-hidden className="text-fg-subtle" />
            <BanRow label={t("live.myTeam")} ids={view.myBans} />
            <BanRow label={t("live.theirTeam")} ids={view.theirBans} />
          </span>
        )}
      </Card>

      {view.benchEnabled && <Bench view={view} />}

      <Teams
        teams={[
          { title: t("live.myTeam"), side: view.side, seats: view.myTeam },
          ...(view.theirTeam.length > 0
            ? [{ title: t("live.theirTeam"), side: opposite(view.side), seats: view.theirTeam }]
            : []),
        ]}
      />
      <ChampSelectBuild view={view} />
      <Callout lines={view.callout} queueId={view.queueId} />
    </div>
  );
}

function Game({ view }: { view: GameView }) {
  const t = useT();
  const catalog = useCatalog();
  const mine = view.teams.findIndex((team) => team.some((seat) => seat.isSelf));
  // Whose team is whose; a spectator gets the sides themselves.
  const teams: TeamTab[] = view.teams.map((seats, index) => {
    const side: Side | null = view.sides ? (index === 0 ? "blue" : "red") : null;
    if (mine === -1)
      return {
        title: side ? t(`live.${side}`) : t("live.team", { n: index + 1 }),
        side: null,
        seats,
      };
    return { title: t(index === mine ? "live.myTeam" : "live.theirTeam"), side, seats };
  });
  return (
    <div className="flex flex-col gap-4">
      <Card className="flex items-center gap-2.5">
        <Swords size={16} strokeWidth={2} className="text-accent-text" aria-hidden />
        <span className="text-[15px] font-semibold text-fg">
          {queueName(view.queueId, "", catalog, t("phase.InProgress"))}
        </span>
        <Badge tone="accent">{t("phase.InProgress")}</Badge>
      </Card>
      <Teams teams={teams} initial={Math.max(0, mine)} />
      <GameBuild view={view} />
      {/* Callout: only a player on one of two sides has an other team to talk about. */}
      {view.sides && mine !== -1 && (
        <Callout lines={view.callout} allies={view.allyCallout} queueId={view.queueId} game />
      )}
    </div>
  );
}

/** The party before the game: who is in it and how they have been playing. */
function Lobby({ view }: { view: LobbyView }) {
  const t = useT();
  const catalog = useCatalog();
  const { navigate } = useShell();
  const phase = useLive((snapshot) => snapshot.phase);
  return (
    <div className="flex flex-col gap-4">
      <Card className="flex flex-wrap items-center gap-2.5">
        <Users size={16} strokeWidth={2} className="text-accent-text" aria-hidden />
        <span className="text-[15px] font-semibold text-fg">
          {queueName(view.queueId, "", catalog, t("phase.Lobby"))}
        </span>
        <Badge tone="accent">{t(`phase.${phase}`)}</Badge>
        {view.custom && <Badge>{t("social.lobbyCustom")}</Badge>}
      </Card>
      <Panel eyebrow={t("social.lobby")} title={t("social.lobbyHint")}>
        <LobbyBoard
          members={view.members}
          onPlayer={(puuid) => navigate({ page: "history", puuid })}
        />
      </Panel>
    </div>
  );
}

function LiveContent() {
  const t = useT();
  const champSelect = useLive((snapshot) => snapshot.champSelect);
  const game = useLive((snapshot) => snapshot.game);
  const lobby = useLive((snapshot) => snapshot.lobby);
  const phase = useLive((snapshot) => snapshot.phase);
  if (champSelect) return <ChampSelect view={champSelect} />;
  if (game) return <Game view={game} />;
  if (lobby) return <Lobby view={lobby} />;
  return (
    <div className="flex flex-col gap-4">
      <EmptyState icon={Swords} title={t("live.idleTitle")} compact>
        <p>{t("live.idle")}</p>
        <p className="mono mt-2 text-[11px] text-fg-subtle">
          {t("overview.phase")} · {t(`phase.${phase}`)}
        </p>
      </EmptyState>
      <BuildLookup />
    </div>
  );
}

export function LivePage() {
  const t = useT();
  return (
    <PageBody>
      <ConnectionGate offline={t("live.offline")}>
        <LiveContent />
      </ConnectionGate>
    </PageBody>
  );
}
