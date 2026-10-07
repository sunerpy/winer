import type { MatchDetail, PlayerLine, RecentMatch, Seat } from "@winer/shared";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { demoBackend } from "../lib/demo";
import { AppStore, StoreContext, catalogOf } from "../lib/store";
import { MatchDetailView, standouts } from "./MatchDetailView";
import { MatchRow } from "./MatchRow";
import { TeamBoard } from "./TeamBoard";

function line(
  name: string,
  score: number | null,
  award: PlayerLine["award"],
  damage: number,
  damageShare: number,
): PlayerLine {
  return {
    puuid: name,
    name: { gameName: name, tagLine: "1" },
    iconId: 1,
    championId: 1,
    championLevel: 18,
    position: null,
    spells: [4, 14],
    items: [0, 0, 0, 0, 0, 0, 0],
    augments: [],
    keystone: 0,
    subStyle: 0,
    kills: 10,
    deaths: 2,
    assists: 8,
    cs: 50,
    gold: 12_000,
    damage,
    damageTaken: 20_000,
    vision: 0,
    largestMultiKill: 2,
    win: award === "mvp",
    remake: false,
    placement: null,
    damageShare,
    killParticipation: 0.6,
    score,
    grade: null,
    award,
    feats: [],
  };
}

const DETAIL: MatchDetail = {
  gameId: 7,
  queueId: 2400,
  gameMode: "KIWI",
  gameVersion: "16.19.821.7343",
  startedAt: 0,
  duration: 1433,
  teams: [
    {
      teamId: 100,
      win: true,
      bans: [],
      kills: 30,
      gold: 60_000,
      towers: 3,
      dragons: 0,
      barons: 0,
      players: [line("ann", 8.6, "mvp", 45_000, 0.31), line("bo", 6.1, null, 20_000, 0.14)],
    },
    {
      teamId: 200,
      win: false,
      bans: [],
      kills: 20,
      gold: 50_000,
      towers: 1,
      dragons: 0,
      barons: 0,
      players: [line("cy", 7.2, "svp", 30_000, 0.4)],
    },
  ],
};

describe("MatchDetailView", () => {
  it("shows each line's score, the MVP and SVP, and the damage share", () => {
    render(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={DETAIL} highlight="bo" />
      </StoreContext>,
    );
    const row = (name: string) => screen.getByText(`${name}#1`).closest("li") as HTMLElement;
    expect(within(row("ann")).getByText("MVP")).toBeInTheDocument();
    expect(within(row("ann")).getByText("8.6")).toBeInTheDocument();
    expect(within(row("ann")).getByText("31%")).toBeInTheDocument();
    expect(within(row("cy")).getByText("SVP")).toBeInTheDocument();
    expect(within(row("bo")).queryByText(/MVP|SVP/)).toBeNull();
    expect(screen.getAllByText("60%")).toHaveLength(3);
    expect(screen.queryByText(/小龙|男爵/), "no dragons or barons on this map").toBeNull();
  });

  it("picks out the game's best values, and nothing everyone ties on", () => {
    expect(standouts(DETAIL), "only damage and score differ").toEqual({
      damage: 45_000,
      score: 8.6,
    });
    const varied: MatchDetail = {
      ...DETAIL,
      teams: [
        {
          ...DETAIL.teams[0]!,
          players: [
            { ...line("ann", 8.6, "mvp", 45_000, 0.31), deaths: 1, gold: 15_000 },
            { ...line("bo", 6.1, null, 20_000, 0.14), damageTaken: 31_000 },
          ],
        },
        DETAIL.teams[1]!,
      ],
    };
    render(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={varied} />
      </StoreContext>,
    );
    const row = (name: string) => screen.getByText(`${name}#1`).closest("li") as HTMLElement;
    const picked = (name: string) =>
      [...row(name).querySelectorAll("[title^='全场']")].map((element) =>
        element.getAttribute("title"),
      );
    expect(picked("ann")).toEqual(["全场死亡最少", "全场评分最高", "全场伤害最高", "全场经济最高"]);
    expect(picked("bo")).toEqual(["全场承伤最高"]);
    expect(picked("cy")).toEqual([]);

    const remake: MatchDetail = {
      ...varied,
      teams: varied.teams.map((team) => ({
        ...team,
        players: team.players.map((player) => ({ ...player, remake: true })),
      })),
    };
    expect(standouts(remake), "a remake picks out nobody").toEqual({});
  });

  it("grades each score and, while titles are on, names what each line earned", () => {
    const graded: MatchDetail = {
      ...DETAIL,
      teams: [
        {
          ...DETAIL.teams[0]!,
          players: DETAIL.teams[0]!.players.map((player, index) => ({
            ...player,
            grade: index === 0 ? 1 : 3,
          })),
        },
        DETAIL.teams[1]!,
      ],
    };
    const { rerender } = render(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={graded} />
      </StoreContext>,
    );
    const row = (name: string) => screen.getByText(`${name}#1`).closest("li") as HTMLElement;
    expect(within(row("ann")).getByTitle("单局评级 S · 人形防御塔")).toHaveTextContent("S");
    expect(within(row("bo")).getByTitle("单局评级 B · 有用之人")).toBeInTheDocument();
    expect(within(row("cy")).queryByTitle(/^单局评级/), "no grade, no letter").toBeNull();
    expect(screen.queryByText("人肉防御塔"), "titles are off").toBeNull();

    rerender(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={graded} titles />
      </StoreContext>,
    );
    // Both of the winners took half their team's damage; only one of them dealt much.
    expect(within(row("ann")).getByText("人肉防御塔")).toBeInTheDocument();
    expect(within(row("bo")).getByText("峡谷保K大师").closest("[title]")).toHaveAttribute(
      "title",
      "KDA 5 以上，伤害占比却不到一成五。",
    );
    expect(within(row("cy")).queryByText(/大师|防御塔/), "a team of one earns nothing").toBeNull();
  });

  it("counts dragons and barons for both sides once either took one", () => {
    const rift: MatchDetail = {
      ...DETAIL,
      teams: [{ ...DETAIL.teams[0]!, dragons: 2 }, DETAIL.teams[1]!],
    };
    render(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={rift} />
      </StoreContext>,
    );
    const headers = screen.getAllByText(/小龙/).map((element) => element.textContent);
    expect(headers).toHaveLength(2);
    expect(headers[0]).toMatch(/推塔 3 · 小龙 2 · 男爵 0$/);
    expect(headers[1]).toMatch(/推塔 1 · 小龙 0 · 男爵 0$/);
  });
});

/** A store that knows two augments and what one of them does. */
function augmentStore(): AppStore {
  const store = new AppStore(demoBackend());
  store.catalog.set(
    catalogOf({
      champions: [],
      items: [],
      spells: [],
      perks: [],
      queues: [{ id: 2400, name: "海克斯大乱斗", gameMode: "KIWI", ranked: false }],
      augments: [
        { id: 1004, name: "回归基本功", icon: "", rarity: "prismatic" },
        { id: 1116, name: "闪现向前", icon: "", rarity: "gold" },
      ],
    }),
  );
  store.augmentDetails.set(new Map([[1004, "你的终极技能已被封印。"]]));
  return store;
}

describe("augments", () => {
  it("name their rarity and say what they do on the scoreboard", () => {
    const withAugments: MatchDetail = {
      ...DETAIL,
      teams: [
        {
          ...DETAIL.teams[0]!,
          players: [{ ...line("ann", 8.6, "mvp", 45_000, 0.31), augments: [1004, 1116] }],
        },
      ],
    };
    render(
      <StoreContext value={augmentStore()}>
        <MatchDetailView detail={withAugments} />
      </StoreContext>,
    );
    expect(screen.getByRole("img", { name: "回归基本功" })).toHaveAttribute(
      "title",
      "回归基本功 · 棱彩\n你的终极技能已被封印。",
    );
    expect(screen.getByRole("img", { name: "闪现向前" })).toHaveAttribute(
      "title",
      "闪现向前 · 金色",
    );
  });

  it("leave the award on the champion's corner in a history row", () => {
    const game = (award: PlayerLine["award"]) => ({
      gameId: 1,
      queueId: 2400,
      gameMode: "KIWI",
      startedAt: 0,
      duration: 1433,
      line: line("ann", 8.6, award, 45_000, 0.31),
      kind: "matched" as const,
    });
    const { rerender } = render(
      <StoreContext value={augmentStore()}>
        <MatchRow game={game("mvp")} now={0} />
      </StoreContext>,
    );
    expect(screen.getByText("MVP")).toHaveAttribute("title", "MVP：胜方本局评分最高");
    rerender(
      <StoreContext value={augmentStore()}>
        <MatchRow game={game("svp")} now={0} />
      </StoreContext>,
    );
    expect(screen.getByText("SVP")).toHaveAttribute("title", "SVP：败方本局评分最高");
    rerender(
      <StoreContext value={augmentStore()}>
        <MatchRow game={game(null)} now={0} />
      </StoreContext>,
    );
    expect(screen.queryByText(/^[MS]VP$/)).toBeNull();
  });

  it("name a history row's feats, the first three as chips and the rest on hover", () => {
    const game = (dense: boolean) => (
      <StoreContext value={augmentStore()}>
        <MatchRow
          dense={dense}
          now={0}
          game={{
            gameId: 1,
            queueId: 2400,
            gameMode: "KIWI",
            startedAt: 0,
            duration: 1433,
            line: {
              ...line("ann", 8.6, "mvp", 45_000, 0.31),
              feats: ["penta", "mostKills", "firstBlood", "mostGold"],
            },
            kind: "matched",
          }}
        />
      </StoreContext>
    );
    const { rerender } = render(game(false));
    const feats = screen.getByLabelText("本局成就");
    expect(within(feats).getByText("五杀")).toHaveAttribute("title", "五杀：拿下了五杀。");
    expect(within(feats).getByText("杀人最多")).toBeInTheDocument();
    expect(within(feats).getByText("一血")).toBeInTheDocument();
    expect(within(feats).getByText("+1")).toHaveAttribute("title", "金币最多");
    rerender(game(true));
    expect(within(screen.getByLabelText("本局成就")).getByText("+2")).toHaveAttribute(
      "title",
      "一血 · 金币最多",
    );
  });

  it("mark each scoreboard line's feats, a multikill by its count", () => {
    const marked: MatchDetail = {
      ...DETAIL,
      teams: [
        {
          ...DETAIL.teams[0]!,
          players: [
            { ...line("ann", 8.6, "mvp", 45_000, 0.31), feats: ["triple", "mostDamage"] },
            line("bo", 6.1, null, 20_000, 0.14),
          ],
        },
        DETAIL.teams[1]!,
      ],
    };
    render(
      <StoreContext value={new AppStore(demoBackend())}>
        <MatchDetailView detail={marked} />
      </StoreContext>,
    );
    const row = (name: string) => screen.getByText(`${name}#1`).closest("li") as HTMLElement;
    const triple = within(row("ann")).getByTitle("三杀：本局最多一次三杀。");
    expect(triple).toHaveTextContent("3");
    expect(within(row("ann")).getByTitle("输出最高：全场对英雄伤害最高。")).toBeInTheDocument();
    expect(within(row("bo")).queryByLabelText("本局成就"), "no feats, no marks").toBeNull();
  });

  it("take the runes' place in a history row", () => {
    const game = {
      gameId: 1,
      queueId: 2400,
      gameMode: "KIWI",
      startedAt: 0,
      duration: 1433,
      line: { ...line("ann", null, null, 1, 0), augments: [1004] },
      kind: "matched" as const,
    };
    render(
      <StoreContext value={augmentStore()}>
        <MatchRow game={game} now={0} />
      </StoreContext>,
    );
    expect(screen.getByRole("img", { name: "回归基本功" })).toBeInTheDocument();
  });
});

/** A seat whose player has `matches` as their latest games. */
function rated(puuid: string, matches: RecentMatch[], isSelf = false): Seat {
  return {
    puuid,
    name: { gameName: puuid, tagLine: "1" },
    championId: 1,
    intent: false,
    position: null,
    spells: [4, 14],
    isSelf,
    premade: null,
    premadeInferred: false,
    rating: null,
    stats: {
      state: "ready",
      puuid,
      name: { gameName: puuid, tagLine: "1" },
      level: 30,
      iconId: 1,
      private: false,
      ranked: { solo: null, flex: null },
      recent: {
        games: matches.length,
        wins: matches.filter((game) => game.win).length,
        kills: 5,
        deaths: 2,
        assists: 7,
        streak: 0,
        matches,
        champions: [],
        score: null,
        source: null,
        family: null,
        away: 0,
      },
    },
  };
}

function played(gameId: number, queueId: number, win: boolean, remake = false): RecentMatch {
  return {
    gameId,
    queueId,
    championId: 1,
    win,
    remake,
    kills: 8,
    deaths: 2,
    assists: 10,
    startedAt: Date.now() - gameId * 3_600_000,
    score: null,
    away: false,
  };
}

describe("TeamBoard", () => {
  it("lines each player's form up with their latest games and opens their history", async () => {
    const store = new AppStore(demoBackend());
    store.catalog.set(
      catalogOf({
        champions: [],
        items: [],
        spells: [],
        perks: [],
        augments: [],
        queues: [
          { id: 2400, name: "海克斯大乱斗", gameMode: "KIWI", ranked: false },
          { id: 420, name: "单双排", gameMode: "CLASSIC", ranked: true },
        ],
      }),
    );
    const onPlayer = vi.fn();
    const hidden: Seat = { ...rated("x", []), puuid: null, name: null, stats: { state: "hidden" } };
    render(
      <StoreContext value={store}>
        <TeamBoard
          seats={[
            rated(
              "ann",
              [played(1, 2400, true), played(2, 420, false), played(3, 2400, false, true)],
              true,
            ),
            hidden,
          ]}
          onPlayer={onPlayer}
        />
      </StoreContext>,
    );
    const tiles = within(screen.getByRole("list", { name: "最近对局" })).getAllByRole("listitem");
    expect(tiles.map((tile) => tile.textContent)).toEqual([
      "8/2/10海克斯",
      "8/2/10排位",
      "8/2/10海克斯",
    ]);
    expect(tiles.map((tile) => /bg-(win|loss)-soft|bg-inset/.exec(tile.className)?.[0])).toEqual([
      "bg-win-soft",
      "bg-loss-soft",
      "bg-inset",
    ]);
    expect(tiles[0]).toHaveAttribute(
      "title",
      expect.stringMatching(/^海克斯大乱斗 · 胜利 · 8\/2\/10 \(9\.0\) · /),
    );
    expect(screen.getByText("隐藏的玩家")).toBeInTheDocument();
    expect(screen.getAllByRole("button"), "a hidden player has no history to open").toHaveLength(1);

    await userEvent.click(screen.getByRole("button", { name: "查看 ann#1 的战绩" }));
    expect(onPlayer).toHaveBeenCalledWith("ann");
  });

  it("shows a seat's grade letter, roast title and quip with its tier", () => {
    const seat: Seat = {
      ...rated("ann", [played(1, 2400, true)]),
      rating: {
        score: 7.8,
        tier: 0,
        tiers: 8,
        label: "峡谷通天代",
        grade: 0,
        title: "版本答案",
        quip: "对面五个人举报代练的水平",
      },
    };
    render(
      <StoreContext value={new AppStore(demoBackend())}>
        <TeamBoard seats={[seat, { ...seat, puuid: "bo", rating: null }]} onPlayer={vi.fn()} />
      </StoreContext>,
    );
    const tier = screen.getByTitle("评分 7.8");
    expect(tier).toHaveTextContent("S+峡谷通天代");
    expect(screen.getByText("版本答案")).toBeInTheDocument();
    expect(screen.getByText("“对面五个人举报代练的水平”")).toBeInTheDocument();
    expect(screen.getAllByText(/峡谷通天代/), "an unrated seat shows none of it").toHaveLength(1);
  });
});
