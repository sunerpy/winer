// The demo client's rewards and profile icon tools (`demo.ts`, the Tools page): grants of the shapes
// measured on a Tencent account (`docs/platform-notes.md`), one of them a real choice, and a
// claim that hands out what the core would.
import type {
  ClaimReport,
  Crest,
  CrestChange,
  GrantChoice,
  IconChange,
  PendingGrant,
  RewardsView,
} from "@winer/shared";

import type { Commands } from "./backend";

type RewardsCommand =
  | "get_pending_rewards"
  | "claim_rewards"
  | "use_default_icon"
  | "set_icon"
  | "use_plain_crest"
  | "set_crest";

const DEFAULT_ICON = 29;
const PLAIN_CREST: Crest = { kind: "prestige", level: 1 };

const reward = (id: string, title: string, quantity = 1) => ({ id, title, quantity, icon: "" });

const GRANTS: PendingGrant[] = [
  {
    id: "demo-grant-icon",
    title: "斗魂觉醒 神之眼 图标",
    created: "2023-08-19T14:20:27.982+00:00",
    rewards: [reward("demo-reward-icon", "斗魂觉醒 神之眼 图标")],
    min: 1,
    max: 1,
  },
  {
    id: "demo-grant-essence",
    title: "750蓝色精萃",
    created: "2026-03-21T14:01:56.987+00:00",
    rewards: [reward("demo-reward-essence", "750蓝色精萃", 750)],
    min: 1,
    max: 1,
  },
  {
    id: "demo-grant-pair",
    title: "Faker之友",
    created: "2026-10-30T15:17:31.055+00:00",
    rewards: [
      reward("demo-reward-poro", "真实伤害魄罗"),
      reward("demo-reward-hat", "时髦的红帽子"),
    ],
    min: 2,
    max: 2,
  },
  {
    id: "demo-grant-choice",
    title: "三选一：守卫皮肤",
    created: "2026-10-31T13:00:00.000+00:00",
    rewards: [
      reward("demo-reward-ward-1", "魄罗守卫"),
      reward("demo-reward-ward-2", "冰雪节守卫"),
      reward("demo-reward-ward-3", "灵魂莲华守卫"),
    ],
    min: 1,
    max: 1,
  },
];

const isChoice = (grant: PendingGrant) => grant.rewards.length > grant.max;

/** The demo's answers to the Tools page's rewards and icon tools. */
export function demoRewardsHandlers(initialIcon: number): {
  [K in RewardsCommand]: (args: Commands[K]["args"]) => Commands[K]["result"];
} {
  let waiting = GRANTS;
  let icon = initialIcon;
  let crest: Crest = { kind: "prestige", level: 7 };
  const view = (): RewardsView => {
    const choices = waiting.filter(isChoice).length;
    return { grants: waiting, claimable: waiting.length - choices, choices };
  };
  const fits = (grant: PendingGrant, choice: GrantChoice | undefined) =>
    choice !== undefined &&
    new Set(choice.rewardIds).size === choice.rewardIds.length &&
    choice.rewardIds.length >= grant.min &&
    choice.rewardIds.length <= grant.max &&
    choice.rewardIds.every((id) => grant.rewards.some((reward) => reward.id === id));
  return {
    get_pending_rewards: view,
    // As the core does: every grant with nothing to choose, and a choice only as picked.
    claim_rewards: ({ choices }) => {
      const report: ClaimReport = {
        claimed: 0,
        failed: [],
        processing: 0,
        left: 0,
        stopped: false,
      };
      waiting = waiting.filter((grant) => {
        const choice = choices.find((pick) => pick.grantId === grant.id);
        if (!isChoice(grant) || fits(grant, choice)) {
          report.claimed += 1;
          return false;
        }
        if (choice)
          report.failed.push({
            grantId: grant.id,
            title: grant.title,
            reason: { kind: "invalid" },
          });
        return true;
      });
      report.left = waiting.length;
      return report;
    },
    use_default_icon: () => {
      const change: IconChange = { previous: icon, current: DEFAULT_ICON };
      icon = DEFAULT_ICON;
      return change;
    },
    set_icon: ({ icon: next }) => {
      const change: IconChange = { previous: icon, current: next };
      icon = next;
      return change;
    },
    use_plain_crest: () => {
      const change: CrestChange = { previous: crest, current: PLAIN_CREST };
      crest = PLAIN_CREST;
      return change;
    },
    set_crest: ({ crest: next }) => {
      const change: CrestChange = { previous: crest, current: next };
      crest = next;
      return change;
    },
  };
}
