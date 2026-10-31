// The rewards the client holds for the player: every waiting grant, one click for all of them, ticks
// for a grant that offers a choice (winer never picks), and the switch that claims after each game.
import type { ClaimFailure, ClaimReport, GrantChoice, PendingGrant, Phase } from "@winer/shared";
import { Gift, RefreshCw } from "lucide-react";
import { useState } from "react";

import { errorCode, errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { type Translate, useLanguage, useT } from "../../lib/i18n";
import { useLive, useSettings, useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import { Button, EmptyState, IconButton, Panel, Row, Skeleton, Toggle, toast } from "../../ui";
import { Art } from "./Art";

/** Where the core claims: out of the queue, champ select and the game. */
const CLAIM_PHASES: readonly Phase[] = ["None", "Lobby", "EndOfGame"];

/** More rewards on offer than the player may take: the player picks. */
export function isChoice(grant: PendingGrant): boolean {
  return grant.rewards.length > grant.max;
}

/** The player's picks fit the grant's rule. */
export function fits(grant: PendingGrant, picked: readonly string[]): boolean {
  return picked.length >= Math.max(grant.min, 1) && picked.length <= grant.max;
}

/** What one click claims: every grant with nothing to choose, and each choice picked as it may be. */
export function claimable(
  grants: readonly PendingGrant[],
  picks: Record<string, string[]>,
): number {
  return grants.filter((grant) => !isChoice(grant) || fits(grant, picks[grant.id] ?? [])).length;
}

/** The report in one line. */
export function reportText(report: ClaimReport, t: Translate): string {
  const parts = [
    report.claimed > 0
      ? t("rewards.report.claimed", { n: report.claimed })
      : t("rewards.report.nothing"),
  ];
  if (report.processing > 0) parts.push(t("rewards.report.processing", { n: report.processing }));
  if (report.failed.length > 0) parts.push(t("rewards.report.failed", { n: report.failed.length }));
  if (report.stopped) parts.push(t("rewards.report.stopped"));
  return parts.join(" · ");
}

function reasonText(failure: ClaimFailure, t: Translate): string {
  const reason = failure.reason;
  return reason.kind === "refused"
    ? t("rewards.reason.refused", { message: reason.message })
    : t(`rewards.reason.${reason.kind}`);
}

/** A reward's own name, its count where the name does not already say it. */
function rewardLabel(title: string, quantity: number): string {
  return quantity > 1 && !title.includes(String(quantity)) ? `${title} ×${quantity}` : title;
}

function GrantRow({
  grant,
  picked,
  onPick,
  day,
}: {
  grant: PendingGrant;
  picked: string[];
  onPick: (picked: string[]) => void;
  day: Intl.DateTimeFormat;
}) {
  const t = useT();
  const choice = isChoice(grant);
  const created = Date.parse(grant.created);
  const toggle = (id: string) =>
    onPick(
      picked.includes(id)
        ? picked.filter((other) => other !== id)
        : // One of one replaces the pick; more of more add to it.
          grant.max === 1
          ? [id]
          : [...picked, id],
    );
  return (
    <li className="flex flex-col gap-1.5 border-b border-border py-2.5 last:border-b-0">
      <div className="flex items-baseline justify-between gap-3">
        <span className="min-w-0 truncate text-[13px] font-medium text-fg">{grant.title}</span>
        <span className="shrink-0 text-[11.5px] text-fg-subtle">
          {choice &&
            (grant.min === grant.max
              ? t("rewards.pick", { n: grant.max })
              : t("rewards.pickRange", { min: grant.min, max: grant.max }))}
          {choice && !Number.isNaN(created) && " · "}
          {!Number.isNaN(created) && t("rewards.granted", { date: day.format(created) })}
        </span>
      </div>
      <div
        className="flex flex-wrap gap-1.5"
        role={choice ? "group" : undefined}
        aria-label={choice ? t("rewards.group", { title: grant.title }) : undefined}
      >
        {grant.rewards.map((reward) => {
          const label = rewardLabel(reward.title, reward.quantity);
          const art = <Art path={reward.icon} alt="" className="size-5 rounded-4" />;
          if (!choice) {
            return (
              <span
                key={reward.id}
                className="flex h-7 items-center gap-1.5 rounded-pill pr-2.5 pl-1 text-[12px] text-fg-muted hairline"
              >
                {art}
                {label}
              </span>
            );
          }
          const checked = picked.includes(reward.id);
          return (
            <button
              key={reward.id}
              type="button"
              role="checkbox"
              aria-checked={checked}
              onClick={() => toggle(reward.id)}
              className={cx(
                "flex h-7 items-center gap-1.5 rounded-pill pr-2.5 pl-1 text-[12px] transition-colors duration-150 hairline",
                checked
                  ? "bg-accent-soft font-medium text-accent-text"
                  : "text-fg-muted hover:text-fg",
              )}
            >
              {art}
              {label}
            </button>
          );
        })}
      </div>
    </li>
  );
}

export function RewardsPanel() {
  const t = useT();
  const language = useLanguage();
  const store = useStore();
  const auto = useSettings().automation.claimRewards;
  const phase = useLive((snapshot) => snapshot.phase);
  const pending = useAsync(() => store.backend.call("get_pending_rewards"), []);
  const [picks, setPicks] = useState<Record<string, string[]>>({});
  const [claiming, setClaiming] = useState(false);
  const [report, setReport] = useState<ClaimReport | null>(null);
  const [named, setNamed] = useState<ReadonlyMap<string, string>>(new Map());
  const day = new Intl.DateTimeFormat(language, { dateStyle: "medium" });
  const grants = pending.data?.grants ?? [];
  const blocked = !CLAIM_PHASES.includes(phase);
  const count = claimable(grants, picks);

  const claim = async () => {
    const choices: GrantChoice[] = grants
      .filter((grant) => isChoice(grant) && fits(grant, picks[grant.id] ?? []))
      .map((grant) => ({ grantId: grant.id, rewardIds: picks[grant.id] ?? [] }));
    setClaiming(true);
    try {
      // The names as listed, for a grant the report can no longer name.
      setNamed(new Map(grants.map((grant) => [grant.id, grant.title])));
      setReport(await store.backend.call("claim_rewards", { choices }));
      setPicks({});
      pending.reload();
    } catch (error) {
      toast(errorCode(error) === "busy" ? t("rewards.blocked") : errorMessage(error), "danger");
    } finally {
      setClaiming(false);
    }
  };

  const switchAuto = (claimRewards: boolean) =>
    void store
      .updateSettings((settings) => ({
        ...settings,
        automation: { ...settings.automation, claimRewards },
      }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  return (
    <Panel eyebrow={t("rewards.title")} className="col-span-12">
      <Row label={t("rewards.pending")} help={t("rewards.pendingHint")}>
        <div className="flex items-center gap-1.5">
          <IconButton
            icon={RefreshCw}
            label={t("rewards.refresh")}
            disabled={pending.loading}
            onClick={() => pending.reload()}
          />
          <Button
            size="sm"
            variant="accent"
            icon={Gift}
            loading={claiming}
            disabled={blocked || count === 0}
            onClick={() => void claim()}
          >
            {t("rewards.claimAll", { n: count })}
          </Button>
        </div>
      </Row>
      <Row label={t("rewards.auto")} help={t("rewards.autoHint")}>
        <Toggle checked={auto} onChange={switchAuto} label={t("rewards.auto")} />
      </Row>
      {blocked && <p className="pt-3 text-[12px] text-fg-muted">{t("rewards.blocked")}</p>}
      {report && (
        <div role="status" className="pt-3 text-[12px] leading-5">
          <p className={report.failed.length > 0 ? "text-danger" : "text-ok-text"}>
            {reportText(report, t)}
          </p>
          {report.failed.length > 0 && (
            <ul className="text-fg-muted">
              {report.failed.map((failure) => (
                <li key={failure.grantId}>
                  {t("rewards.failure", {
                    title: failure.title || named.get(failure.grantId) || failure.grantId,
                    reason: reasonText(failure, t),
                  })}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {pending.loading && !pending.data ? (
        <Skeleton className="mt-3 h-24" />
      ) : pending.error ? (
        <p className="pt-3 text-[12px] text-danger">{errorMessage(pending.error)}</p>
      ) : grants.length === 0 ? (
        <EmptyState compact icon={Gift} title={t("rewards.none")} className="mt-3" />
      ) : (
        <ul className="mt-1">
          {grants.map((grant) => (
            <GrantRow
              key={grant.id}
              grant={grant}
              picked={picks[grant.id] ?? []}
              onPick={(picked) => setPicks((all) => ({ ...all, [grant.id]: picked }))}
              day={day}
            />
          ))}
        </ul>
      )}
    </Panel>
  );
}
