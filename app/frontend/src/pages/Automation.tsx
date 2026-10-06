import type { Audience, Automation, CalloutRule, Mode, Scopes } from "@winer/shared";
import { type ReactNode, useState } from "react";

import { ChampionList, ChampionPoolEditor } from "../game/ChampionPoolEditor";
import { errorMessage } from "../lib/backend";
import { cx } from "../lib/cx";
import { useLanguage, useT } from "../lib/i18n";
import { APPLICABLE, MODES, MODE_LABEL, type ScopedRule, everywhere, toggled } from "../lib/modes";
import { useLive, useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { useShell } from "../shell/navigation";
import {
  Button,
  Card,
  CommitInput,
  Panel,
  Row,
  Segmented,
  Skeleton,
  Slider,
  Toggle,
  toast,
} from "../ui";
import { ItemSetsPanel, LoadoutPanel } from "./automation/LoadoutRules";
import { PageBody } from "./common";

const WISHLIST_LIMIT = 20;
/** The core's default line (`callout::template`), shown as the placeholder of a blank template. */
const DEFAULT_TEMPLATE: Record<"zh-CN" | "en", string> = {
  "zh-CN": "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
  en: "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
};

/** Where a rule acts: one switch per kind of game it can act in at all. */
function ScopeRow({
  rule,
  scopes,
  focus,
  onChange,
}: {
  rule: ScopedRule;
  scopes: Scopes;
  /** The mode the page is looking at, marked among the chips. */
  focus: Mode | null;
  onChange: (scopes: Scopes) => void;
}) {
  const t = useT();
  const modes = scopes[rule];
  const help = everywhere(rule, modes)
    ? t("auto.scopeEverywhere")
    : modes.length === 0
      ? t("auto.scopeNone")
      : t("auto.scopeSome");
  return (
    <Row label={t("auto.scope")} help={help}>
      <div role="group" aria-label={t("auto.scope")} className="flex flex-wrap justify-end gap-1">
        {APPLICABLE[rule].map((mode) => {
          const on = modes.includes(mode);
          return (
            <button
              key={mode}
              type="button"
              aria-pressed={on}
              onClick={() => onChange({ ...scopes, [rule]: toggled(rule, modes, mode) })}
              className={cx(
                "h-6 rounded-pill px-2.5 text-[11.5px] transition-colors duration-150 hairline",
                on ? "bg-accent-soft font-medium text-accent-text" : "text-fg-subtle hover:text-fg",
                focus === mode && "ring-1 ring-accent",
              )}
            >
              {t(MODE_LABEL[mode])}
            </button>
          );
        })}
      </div>
    </Row>
  );
}

/** Which rules act in `mode`, and which have no part in it, said in one or two sentences. */
function ModeSummary({ automation, mode }: { automation: Automation; mode: Mode | null }) {
  const t = useT();
  const language = useLanguage();
  if (mode === null) return <p className="text-[11.5px] text-fg-subtle">{t("auto.byModeIntro")}</p>;
  const list = new Intl.ListFormat(language, { type: "conjunction" });
  const rules: { rule: ScopedRule; label: string; on: boolean }[] = [
    { rule: "accept", label: t("auto.accept"), on: automation.accept.enabled },
    { rule: "pick", label: t("auto.pick"), on: automation.pick.enabled },
    { rule: "ban", label: t("auto.ban"), on: automation.ban.enabled },
    { rule: "callout", label: t("auto.calloutAuto"), on: automation.callout.auto },
    { rule: "bench", label: t("auto.bench"), on: automation.bench.enabled },
    { rule: "playAgain", label: t("auto.playAgain"), on: automation.playAgain },
    { rule: "loadout", label: t("loadout.rule"), on: automation.loadout.enabled },
    { rule: "itemSets", label: t("loadout.itemSets"), on: automation.itemSets },
  ];
  const name = t(MODE_LABEL[mode]);
  const active = rules.filter(
    ({ rule, on }) =>
      on && APPLICABLE[rule].includes(mode) && automation.scopes[rule].includes(mode),
  );
  const unrelated = rules.filter(({ rule }) => !APPLICABLE[rule].includes(mode));
  return (
    <p className="text-[11.5px] text-fg-subtle">
      {active.length > 0
        ? t("auto.inMode", { mode: name, rules: list.format(active.map(({ label }) => label)) })
        : t("auto.inModeNothing", { mode: name })}
      {unrelated.length > 0 &&
        ` ${t("auto.notInMode", { mode: name, rules: list.format(unrelated.map(({ label }) => label)) })}`}
    </p>
  );
}

/** The lines as they would be sent, rendered by the core from the user's own form. */
function CalloutPreview({ rule }: { rule: CalloutRule }) {
  const t = useT();
  const store = useStore();
  // The language and the titles shape the lines too; the window's own copy may be newer than the
  // core's while a change is being saved.
  const general = useSettings().general;
  const connected = useLive((snapshot) => snapshot.connection.status === "connected");
  const preview = useAsync(
    () => store.backend.call("preview_callout", { rule, general }),
    [JSON.stringify(rule), JSON.stringify(general), connected],
    connected,
  );
  return (
    <div className="flex flex-col gap-2 pt-1 pb-3">
      <p className="text-[14px] font-medium text-fg">{t("auto.preview")}</p>
      <div className="rounded-6 bg-inset px-3 py-2 hairline">
        {!connected ? (
          <p className="text-[12px] text-fg-subtle">{t("auto.previewOffline")}</p>
        ) : preview.data ? (
          <ol className="flex flex-col gap-1">
            {preview.data.map((line, index) => (
              <li key={index} className="text-[12.5px] leading-5 break-words text-fg">
                {line}
              </li>
            ))}
          </ol>
        ) : preview.error ? (
          <p className="text-[12px] text-fg-subtle">{errorMessage(preview.error)}</p>
        ) : (
          <div className="flex flex-col gap-1.5 py-0.5">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        )}
      </div>
      <p className="text-[12px] leading-4 text-fg-muted">{t("auto.previewHint")}</p>
    </div>
  );
}

function CalloutPanel({
  rule,
  onChange,
  scope,
}: {
  rule: CalloutRule;
  onChange: (change: (rule: CalloutRule) => CalloutRule) => void;
  scope: ReactNode;
}) {
  const t = useT();
  const language = useSettings().general.language;
  const { openSettings } = useShell();
  return (
    <Panel eyebrow={t("auto.section.callout")} className="col-span-12">
      <Row label={t("auto.calloutAuto")} help={t("auto.calloutAutoHint")}>
        <Toggle
          checked={rule.auto}
          onChange={(auto) => onChange((value) => ({ ...value, auto }))}
          label={t("auto.calloutAuto")}
        />
      </Row>
      {scope}
      <Row label={t("auto.calloutAudience")}>
        <Segmented<Audience>
          size="sm"
          label={t("auto.calloutAudience")}
          value={rule.audience}
          options={[
            { value: "team", label: t("auto.audience.team") },
            { value: "me", label: t("auto.audience.me") },
          ]}
          onChange={(audience) => onChange((value) => ({ ...value, audience }))}
        />
      </Row>
      <Row label={t("auto.calloutSelf")}>
        <Toggle
          checked={rule.includeSelf}
          onChange={(includeSelf) => onChange((value) => ({ ...value, includeSelf }))}
          label={t("auto.calloutSelf")}
        />
      </Row>
      <Row label={t("auto.scheme")} help={t("auto.schemeHint")}>
        <span className="flex items-center gap-2">
          <span className="text-[12.5px] font-medium text-fg">
            {t(`rating.scheme.${rule.tiers}`)}
          </span>
          <Button size="sm" variant="ghost" onClick={() => openSettings("rating")}>
            {t("auto.schemeEdit")}
          </Button>
        </span>
      </Row>
      <Row label={t("auto.header")} help={t("auto.headerHint")} htmlFor="callout-header">
        <CommitInput
          id="callout-header"
          value={rule.header}
          maxLength={200}
          onCommit={(header) => onChange((value) => ({ ...value, header }))}
          className="w-[280px]"
        />
      </Row>
      <div className="flex flex-col gap-2 py-3">
        <label htmlFor="callout-template" className="text-[14px] font-medium text-fg">
          {t("auto.template")}
        </label>
        <div className="flex items-center gap-2">
          <CommitInput
            id="callout-template"
            value={rule.template}
            maxLength={200}
            placeholder={DEFAULT_TEMPLATE[language]}
            onCommit={(template) => onChange((value) => ({ ...value, template }))}
            className="min-w-0 flex-1"
          />
          <Button
            size="sm"
            variant="ghost"
            disabled={!rule.template && !rule.header}
            onClick={() => onChange((value) => ({ ...value, header: "", template: "" }))}
          >
            {t("auto.reset")}
          </Button>
        </div>
        <p className="text-[12px] leading-4 text-fg-muted">{t("auto.templateHint")}</p>
      </div>
      <CalloutPreview rule={rule} />
    </Panel>
  );
}

export function AutomationPage() {
  const t = useT();
  const store = useStore();
  const automation = useSettings().automation;
  const [focus, setFocus] = useState<Mode | null>(null);
  const save = (change: (automation: Automation) => Automation) =>
    void store
      .updateSettings((settings) => ({ ...settings, automation: change(settings.automation) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  const scope = (rule: ScopedRule) => (
    <ScopeRow
      rule={rule}
      scopes={automation.scopes}
      focus={focus}
      onChange={(scopes) => save((value) => ({ ...value, scopes }))}
    />
  );
  /** A panel shows while the page looks at every mode or at one its rule can act in. */
  const shown = (rule: ScopedRule) => focus === null || APPLICABLE[rule].includes(focus);
  /** A section that only some kinds of game have says which in its eyebrow. */
  const eyebrow = (title: string, rule: ScopedRule) =>
    APPLICABLE[rule].length === MODES.length
      ? title
      : `${title} · ${APPLICABLE[rule].map((mode) => t(MODE_LABEL[mode])).join(" / ")}`;

  return (
    <PageBody className="grid grid-cols-12 gap-3">
      <Card className="col-span-12 flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-3">
          <span className="text-[12.5px] font-medium text-fg">{t("auto.byMode")}</span>
          <Segmented
            size="sm"
            label={t("auto.byMode")}
            value={focus ?? "all"}
            options={[
              { value: "all", label: t("auto.byModeAll") },
              ...MODES.map((mode) => ({ value: mode, label: t(MODE_LABEL[mode]) })),
            ]}
            onChange={(value) => setFocus(value === "all" ? null : value)}
          />
        </div>
        <ModeSummary automation={automation} mode={focus} />
      </Card>

      {shown("accept") && (
        <Panel eyebrow={t("auto.section.match")} className="col-span-12 @[900px]:col-span-6">
          <Row label={t("auto.accept")} help={t("auto.acceptHint")}>
            <Toggle
              checked={automation.accept.enabled}
              onChange={(enabled) =>
                save((value) => ({ ...value, accept: { ...value.accept, enabled } }))
              }
              label={t("auto.accept")}
            />
          </Row>
          {scope("accept")}
          <Row label={t("auto.acceptDelay")}>
            <Slider
              label={t("auto.acceptDelay")}
              min={0}
              max={8000}
              step={500}
              value={automation.accept.delayMs}
              format={(ms) => t("common.seconds", { n: (ms / 1000).toFixed(1) })}
              onChange={(delayMs) =>
                save((value) => ({ ...value, accept: { ...value.accept, delayMs } }))
              }
            />
          </Row>
        </Panel>
      )}

      {shown("playAgain") && (
        <Panel eyebrow={t("auto.section.postGame")} className="col-span-12 @[900px]:col-span-6">
          <Row label={t("auto.playAgain")} help={t("auto.playAgainHint")}>
            <Toggle
              checked={automation.playAgain}
              onChange={(playAgain) => save((value) => ({ ...value, playAgain }))}
              label={t("auto.playAgain")}
            />
          </Row>
          {scope("playAgain")}
        </Panel>
      )}

      {shown("pick") && (
        <Panel eyebrow={eyebrow(t("auto.section.pick"), "pick")} className="col-span-12">
          <Row label={t("auto.pick")} help={t("auto.pickHint")}>
            <Toggle
              checked={automation.pick.enabled}
              onChange={(enabled) =>
                save((value) => ({ ...value, pick: { ...value.pick, enabled } }))
              }
              label={t("auto.pick")}
            />
          </Row>
          {scope("pick")}
          <Row label={t("auto.lockIn")}>
            <Segmented
              size="sm"
              label={t("auto.lockIn")}
              value={automation.pick.lockIn ? "lock" : "hover"}
              options={[
                { value: "lock", label: t("auto.lockInLock") },
                { value: "hover", label: t("auto.lockInHover") },
              ]}
              onChange={(mode) =>
                save((value) => ({ ...value, pick: { ...value.pick, lockIn: mode === "lock" } }))
              }
            />
          </Row>
          <Row label={t("auto.declare")} help={t("auto.declareHint")}>
            <Toggle
              checked={automation.pick.declareIntent}
              onChange={(declareIntent) =>
                save((value) => ({ ...value, pick: { ...value.pick, declareIntent } }))
              }
              label={t("auto.declare")}
            />
          </Row>
          <Card padding="sm" className="mt-3 bg-inset/50">
            <ChampionPoolEditor
              label={t("auto.pool")}
              pool={automation.pick.champions}
              onChange={(champions) =>
                save((value) => ({ ...value, pick: { ...value.pick, champions } }))
              }
            />
          </Card>
        </Panel>
      )}

      {shown("ban") && (
        <Panel eyebrow={eyebrow(t("auto.section.ban"), "ban")} className="col-span-12">
          <Row label={t("auto.ban")} help={t("auto.banHint")}>
            <Toggle
              checked={automation.ban.enabled}
              onChange={(enabled) =>
                save((value) => ({ ...value, ban: { ...value.ban, enabled } }))
              }
              label={t("auto.ban")}
            />
          </Row>
          {scope("ban")}
          <Card padding="sm" className="mt-3 bg-inset/50">
            <ChampionPoolEditor
              label={t("auto.pool")}
              pool={automation.ban.champions}
              onChange={(champions) =>
                save((value) => ({ ...value, ban: { ...value.ban, champions } }))
              }
            />
          </Card>
        </Panel>
      )}

      {shown("loadout") && (
        <LoadoutPanel
          eyebrow={eyebrow(t("loadout.section"), "loadout")}
          rule={automation.loadout}
          onChange={(change) => save((value) => ({ ...value, loadout: change(value.loadout) }))}
          scope={scope("loadout")}
        />
      )}

      {shown("itemSets") && (
        <ItemSetsPanel
          eyebrow={eyebrow(t("loadout.itemSetsSection"), "itemSets")}
          enabled={automation.itemSets}
          onChange={(itemSets) => save((value) => ({ ...value, itemSets }))}
          scope={scope("itemSets")}
        />
      )}

      <CalloutPanel
        rule={automation.callout}
        onChange={(change) => save((value) => ({ ...value, callout: change(value.callout) }))}
        scope={scope("callout")}
      />

      {shown("bench") && (
        <Panel eyebrow={eyebrow(t("auto.section.bench"), "bench")} className="col-span-12">
          <Row label={t("auto.bench")} help={t("auto.benchHint")}>
            <Toggle
              checked={automation.bench.enabled}
              onChange={(enabled) =>
                save((value) => ({ ...value, bench: { ...value.bench, enabled } }))
              }
              label={t("auto.bench")}
            />
          </Row>
          {scope("bench")}
          <Card padding="sm" className="mt-3 bg-inset/50">
            <ChampionList
              list={automation.bench.champions}
              limit={WISHLIST_LIMIT}
              hint={t("auto.wishlistHint")}
              leading={
                <span className="text-[12.5px] font-medium text-fg">{t("auto.wishlist")}</span>
              }
              onChange={(champions) =>
                save((value) => ({ ...value, bench: { ...value.bench, champions } }))
              }
            />
          </Card>
        </Panel>
      )}
    </PageBody>
  );
}
