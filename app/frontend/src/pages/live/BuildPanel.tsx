// The build panel (配装推荐): what players take on a champion, from public statistics, in champ
// select, during the game and from a champion search. The numbers come from the core; runes, spells
// and item sets reach the client only when the user asks.
import type {
  AugmentOption,
  Build,
  ChampSelectView,
  GameView,
  ItemOption,
  Matchup,
  Mode,
  PageOutcome,
  Position,
  Rates,
  RuneOption,
  SkillOrder,
  SpellOption,
} from "@winer/shared";
import { positionLabel } from "@winer/shared";
import { Check, ListPlus, Search, Sparkles, Swords } from "lucide-react";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";

import { AssetIcon, AugmentIcon, ChampionIcon } from "../../game/icons";
import { errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { type MessageKey, useLanguage, useT } from "../../lib/i18n";
import { MODE_LABEL, modeOf } from "../../lib/modes";
import { useAugmentDetails, useCatalog, useSettings, useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import {
  Badge,
  Button,
  EmptyState,
  ErrorNote,
  Input,
  Panel,
  Popover,
  Segmented,
  Skeleton,
} from "../../ui";
import {
  type BuildTab,
  LANES,
  LOOKUP_MODES,
  SOURCE_LABEL,
  augmentGroups,
  availableTabs,
  defaultTab,
  hasNumbers,
  isRift,
  rate,
  rateTone,
  tabsOf,
} from "./builds";

/** Where the panel is: champ select can take spells, the game only reads, a lookup is idle. */
export type BuildContext = "champSelect" | "game" | "lookup";

const TAB_LABEL: Record<BuildTab, MessageKey> = {
  items: "loadout.tab.items",
  runes: "loadout.tab.runes",
  spells: "loadout.tab.spells",
  skills: "loadout.tab.skills",
  matchups: "loadout.tab.matchups",
  augments: "loadout.tab.augments",
};

/** How many options of a kind are worth a row. */
const ROWS = 6;

/** An item, spell or rune named for the eye and the screen reader, with or without its icon. */
function Asset({
  kind,
  id,
  size,
}: {
  kind: "items" | "spells" | "perks";
  id: number;
  size: number;
}) {
  const name = useCatalog()?.[kind].get(id)?.name || `#${id}`;
  return (
    <span role="img" aria-label={name} title={name} className="inline-flex shrink-0">
      <AssetIcon kind={kind} id={id} size={size} />
    </span>
  );
}

/** Pick rate, win rate (or Arena's finishes) and games, as small labelled readouts. */
function RateCells({ rates, arena = false }: { rates: Rates; arena?: boolean }) {
  const t = useT();
  const language = useLanguage();
  const cell = (label: string, value: ReactNode) => (
    <span className="flex flex-col items-end leading-4">
      <span className="text-[10.5px] text-fg-subtle">{label}</span>
      {value}
    </span>
  );
  return (
    <span className="ml-auto flex shrink-0 items-center gap-4">
      {cell(
        t("loadout.pick"),
        <span className="mono text-[12px] text-fg">{rate(rates.pick)}</span>,
      )}
      {arena && rates.placement !== null
        ? cell(
            t("loadout.placementShort"),
            <span className="mono text-[12px] text-fg">{rates.placement.toFixed(2)}</span>,
          )
        : cell(
            t("loadout.win"),
            <span className={cx("mono text-[12px]", rateTone(rates.win))}>{rate(rates.win)}</span>,
          )}
      {rates.games !== null &&
        cell(
          t("loadout.games"),
          <span className="mono text-[12px] text-fg-muted">
            {rates.games.toLocaleString(language)}
          </span>,
        )}
    </span>
  );
}

function Row({ children }: { children: ReactNode }) {
  return (
    <li className="flex min-h-11 flex-wrap items-center gap-x-3 gap-y-2 rounded-6 bg-inset px-2.5 py-1.5 hairline">
      {children}
    </li>
  );
}

/** What became of the last thing the user applied: said in the panel, next to the button. */
interface Outcome {
  key: string;
  ok: boolean;
  text: string;
}

function OutcomeLine({ outcome, at }: { outcome: Outcome | null; at: string }) {
  if (!outcome || outcome.key !== at) return null;
  return (
    <p
      role="status"
      className={cx(
        "flex w-full items-start gap-1.5 text-[12px] leading-4",
        outcome.ok ? "text-ok-text" : "text-danger",
      )}
    >
      {outcome.ok && <Check size={13} strokeWidth={2.4} aria-hidden className="mt-px shrink-0" />}
      {outcome.text}
    </p>
  );
}

/** One click at a time, and what it came to. */
function useActions() {
  const [busy, setBusy] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const run = async (key: string, action: () => Promise<string>) => {
    setBusy(key);
    try {
      setOutcome({ key, ok: true, text: await action() });
    } catch (error) {
      setOutcome({ key, ok: false, text: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  };
  return { busy, outcome, run };
}

type Actions = ReturnType<typeof useActions>;

function ItemsTab({
  build,
  lane,
  actions,
}: {
  build: Build;
  lane: Position | null;
  actions: Actions;
}) {
  const t = useT();
  const store = useStore();
  const groups: [MessageKey, ItemOption[]][] = [
    ["loadout.starting", build.starting],
    ["loadout.boots", build.boots],
    ["loadout.core", build.core],
  ];
  const arena = build.mode === "arena";
  return (
    <div className="flex flex-col gap-3">
      {groups
        .filter(([, options]) => options.length > 0)
        .map(([label, options]) => (
          <section key={label} aria-label={t(label)}>
            <h3 className="mb-1.5 text-[12px] font-medium text-fg-muted">{t(label)}</h3>
            <ul className="flex flex-col gap-1">
              {options.slice(0, label === "loadout.core" ? ROWS : 3).map((option, index) => (
                <Row key={index}>
                  <span className="flex items-center gap-1">
                    {option.items.map((id, at) => (
                      <Asset key={at} kind="items" id={id} size={28} />
                    ))}
                  </span>
                  <RateCells rates={option.rates} arena={arena} />
                </Row>
              ))}
            </ul>
          </section>
        ))}
      {build.late.length > 0 && (
        <section aria-label={t("loadout.late")}>
          <h3 className="mb-1.5 text-[12px] font-medium text-fg-muted">{t("loadout.late")}</h3>
          <ul className="flex flex-wrap gap-1.5">
            {build.late.slice(0, 10).map((option) => {
              const id = option.items[0] ?? 0;
              return (
                <li
                  key={id}
                  className="flex flex-col items-center gap-0.5 rounded-6 bg-inset px-1.5 pt-1.5 pb-1 hairline"
                >
                  <Asset kind="items" id={id} size={28} />
                  <span className="mono text-[10.5px] text-fg-muted">
                    {rate(option.rates.pick)}
                  </span>
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <div className="flex flex-wrap items-center gap-2 pt-1">
        <Button
          size="sm"
          icon={ListPlus}
          loading={actions.busy === "itemSet"}
          disabled={actions.busy !== null}
          onClick={() =>
            void actions.run("itemSet", async () => {
              await store.backend.call("write_item_set", {
                championId: build.championId,
                mode: build.mode,
                lane,
              });
              return t("loadout.itemSetWritten");
            })
          }
        >
          {t("loadout.writeItemSet")}
        </Button>
        <span className="text-[11.5px] text-fg-subtle">{t("loadout.itemSetsNote")}</span>
        <OutcomeLine outcome={actions.outcome} at="itemSet" />
      </div>
    </div>
  );
}

function RuneRow({
  option,
  index,
  championId,
  canApply,
  actions,
}: {
  option: RuneOption;
  index: number;
  championId: number;
  canApply: boolean;
  actions: Actions;
}) {
  const t = useT();
  const store = useStore();
  const { page } = option;
  const key = `runes-${index}`;
  const said = (outcome: PageOutcome) =>
    outcome === "written" ? t("loadout.runesWritten") : t("loadout.runesNoPage");
  return (
    <Row>
      <span className="flex items-center gap-1">
        <Asset kind="perks" id={page.primaryStyle} size={16} />
        <Asset kind="perks" id={page.perks[0] ?? 0} size={30} />
        {page.perks.slice(1, 4).map((id, at) => (
          <Asset key={at} kind="perks" id={id} size={20} />
        ))}
      </span>
      <span className="flex items-center gap-1">
        <Asset kind="perks" id={page.subStyle} size={16} />
        {page.perks.slice(4, 6).map((id, at) => (
          <Asset key={at} kind="perks" id={id} size={20} />
        ))}
      </span>
      <span className="flex items-center gap-0.5">
        {page.perks.slice(6).map((id, at) => (
          <Asset key={at} kind="perks" id={id} size={15} />
        ))}
      </span>
      <RateCells rates={option.rates} />
      {canApply && (
        <Button
          size="sm"
          icon={Sparkles}
          loading={actions.busy === key}
          disabled={actions.busy !== null}
          onClick={() =>
            void actions.run(key, async () =>
              said(await store.backend.call("apply_runes", { championId, page })),
            )
          }
        >
          {t("loadout.applyRunes")}
        </Button>
      )}
      <OutcomeLine outcome={actions.outcome} at={key} />
    </Row>
  );
}

function SpellRow({
  option,
  index,
  canApply,
  actions,
}: {
  option: SpellOption;
  index: number;
  canApply: boolean;
  actions: Actions;
}) {
  const t = useT();
  const store = useStore();
  const key = `spells-${index}`;
  return (
    <Row>
      <span className="flex items-center gap-1">
        {option.spells.map((id) => (
          <Asset key={id} kind="spells" id={id} size={28} />
        ))}
      </span>
      <RateCells rates={option.rates} />
      {canApply && (
        <Button
          size="sm"
          loading={actions.busy === key}
          disabled={actions.busy !== null}
          onClick={() =>
            void actions.run(key, async () => {
              await store.backend.call("apply_spells", { spells: option.spells });
              return t("loadout.spellsApplied");
            })
          }
        >
          {t("loadout.applySpells")}
        </Button>
      )}
      <OutcomeLine outcome={actions.outcome} at={key} />
    </Row>
  );
}

function SkillRow({ order, arena }: { order: SkillOrder; arena: boolean }) {
  const t = useT();
  return (
    <Row>
      <span className="flex flex-col gap-1">
        <span className="flex items-center gap-1.5 text-[11px] text-fg-subtle">
          {t("loadout.priority")}
          <span className="mono text-[13px] font-semibold text-fg">
            {order.priority.join(" > ")}
          </span>
        </span>
        {order.sequence.length > 0 && (
          <ol
            aria-label={t("loadout.sequence", { n: order.sequence.length })}
            className="flex flex-wrap gap-0.5"
          >
            {order.sequence.map((ability, level) => (
              <li
                key={level}
                title={t("common.level", { level: level + 1 })}
                className={cx(
                  "mono grid size-5 place-items-center rounded-4 text-[10.5px] font-medium",
                  ability === "R" ? "bg-accent-soft text-accent-text" : "bg-inset2 text-fg",
                )}
              >
                {ability}
              </li>
            ))}
          </ol>
        )}
      </span>
      <RateCells rates={order.rates} arena={arena} />
    </Row>
  );
}

function MatchupList({ title, matchups }: { title: string; matchups: Matchup[] }) {
  const t = useT();
  const catalog = useCatalog();
  return (
    <section aria-label={title} className="min-w-0 flex-1">
      <h3 className="mb-1.5 text-[12px] font-medium text-fg-muted">{title}</h3>
      {matchups.length === 0 ? (
        <p className="text-[12px] text-fg-subtle">{t("loadout.empty")}</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {matchups.map((matchup) => {
            const champion = catalog?.champions.get(matchup.championId);
            return (
              <li
                key={matchup.championId}
                className="flex items-center gap-2 rounded-6 bg-inset px-2 py-1 hairline"
              >
                <ChampionIcon id={matchup.championId} size={24} />
                <span className="min-w-0 flex-1 truncate text-[12.5px] text-fg">
                  {champion?.name ?? `#${matchup.championId}`}
                </span>
                <span className={cx("mono text-[12px]", rateTone(matchup.rates.win))}>
                  {rate(matchup.rates.win)}
                </span>
                {matchup.rates.games !== null && (
                  <span className="mono w-16 text-right text-[11px] text-fg-subtle">
                    {t("common.games", { n: matchup.rates.games })}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function AugmentRow({ option, source }: { option: AugmentOption; source: string }) {
  const t = useT();
  const augment = useCatalog()?.augments.get(option.id);
  const description = useAugmentDetails()?.get(option.id);
  const { rates } = option;
  return (
    <li className="flex items-start gap-2.5 rounded-6 bg-inset px-2.5 py-2 hairline">
      <AugmentIcon id={option.id} size={28} />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex flex-wrap items-center gap-1.5">
          <span className="text-[13px] font-medium text-fg">
            {augment?.name ?? `#${option.id}`}
          </span>
          {option.tier && (
            <Badge
              tone={option.tier === "S" ? "accent" : option.tier === "A" ? "win" : "neutral"}
              title={t("loadout.augmentTier", { source })}
            >
              <span className="mono">{option.tier}</span>
            </Badge>
          )}
        </span>
        {description && (
          <span className="text-[11.5px] leading-4 whitespace-pre-line text-fg-muted">
            {description}
          </span>
        )}
      </span>
      <span className="flex shrink-0 flex-col items-end gap-0.5 text-[11px] leading-4">
        {rates.placement !== null && (
          <span className="mono text-fg" title={t("loadout.placementHint")}>
            {t("loadout.placement", { n: rates.placement.toFixed(2) })}
          </span>
        )}
        {rates.first !== null && (
          <span className="mono text-fg-muted">{t("loadout.first", { p: rate(rates.first) })}</span>
        )}
        <span className="mono text-fg-subtle">
          {t("loadout.pick")} {rate(rates.pick)}
        </span>
      </span>
    </li>
  );
}

function AugmentsTab({ build, source }: { build: Build; source: string }) {
  const t = useT();
  const catalog = useCatalog();
  const details = useAugmentDetails();
  const [filter, setFilter] = useState("");
  const groups = useMemo(
    () => augmentGroups(build.augments, catalog?.augments, filter, details),
    [build.augments, catalog, filter, details],
  );
  return (
    <div className="flex flex-col gap-3">
      <Input
        icon={Search}
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        placeholder={t("loadout.augmentFilter")}
        aria-label={t("loadout.augmentFilter")}
        className="w-full max-w-[320px]"
      />
      {build.mode === "arena" && (
        <p className="text-[11.5px] text-fg-subtle">{t("loadout.placementHint")}</p>
      )}
      {groups.length === 0 ? (
        <p className="py-4 text-center text-[12px] text-fg-subtle">{t("loadout.augmentNoMatch")}</p>
      ) : (
        groups.map((group) => (
          <section key={group.rarity} aria-label={t(`augment.${group.rarity}`)}>
            <h3 className="mb-1.5 flex items-center gap-1.5 text-[12px] font-medium text-fg-muted">
              <span
                aria-hidden
                className="size-2 rounded-full"
                style={{ background: `var(--rarity-${group.rarity})` }}
              />
              {t(`augment.${group.rarity}`)}
              <span className="mono text-[11px] text-fg-subtle">{group.augments.length}</span>
            </h3>
            <ul className="flex flex-col gap-1">
              {group.augments.map((option) => (
                <AugmentRow key={option.id} option={option} source={source} />
              ))}
            </ul>
          </section>
        ))
      )}
    </div>
  );
}

function BuildSkeleton() {
  return (
    <div className="flex flex-col gap-1.5" aria-hidden>
      {[0, 1, 2, 3].map((row) => (
        <Skeleton key={row} className="h-11 w-full" />
      ))}
    </div>
  );
}

/** The numbers for one champion, mode and lane, in their sections. */
function BuildBody({
  championId,
  mode,
  lane,
  context,
  onBuild,
}: {
  championId: number;
  mode: Mode;
  lane: Position | null;
  context: BuildContext;
  onBuild: (build: Build | null) => void;
}) {
  const t = useT();
  const language = useLanguage();
  const store = useStore();
  const riftSource = useSettings().builds.riftSource;
  // Lanes are the Rift's; elsewhere the numbers are the champion's whatever its seat says.
  const [chosen, setChosen] = useState<Position | null>(isRift(mode) ? lane : null);
  const [tab, setTab] = useState<BuildTab | null>(null);
  const actions = useActions();
  const answer = useAsync(
    () => store.backend.call("get_build", { championId, mode, lane: chosen }),
    [championId, mode, chosen, riftSource],
  );
  const build = answer.data;
  // The header says where the numbers on screen come from, so it follows the answer shown.
  useEffect(() => {
    onBuild(answer.error ? null : (answer.data ?? null));
  }, [answer.data, answer.error, onBuild]);
  const tabs = build ? availableTabs(build) : tabsOf(mode);
  const shown = tab && tabs.includes(tab) ? tab : defaultTab(tabs, mode, context === "game");
  const source = build ? t(SOURCE_LABEL[build.source]) : "";

  if (answer.error && !answer.loading) {
    return (
      <ErrorNote
        title={t("loadout.failed")}
        detail={errorMessage(answer.error)}
        retryLabel={t("common.retry")}
        onRetry={answer.reload}
      />
    );
  }
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Segmented<BuildTab>
          size="sm"
          label={t("loadout.tabs")}
          value={shown}
          options={tabs.map((value) => ({ value, label: t(TAB_LABEL[value]) }))}
          onChange={setTab}
        />
        {isRift(mode) && (
          <Segmented<Position>
            size="sm"
            label={t("loadout.lane")}
            value={chosen ?? build?.lane ?? null}
            options={LANES.map((value) => ({ value, label: positionLabel(value, language) }))}
            onChange={setChosen}
          />
        )}
      </div>
      {!build || answer.loading ? (
        <BuildSkeleton />
      ) : (
        <div>
          {shown === "items" && (
            <ItemsTab build={build} lane={chosen ?? build.lane} actions={actions} />
          )}
          {shown === "runes" && (
            <ul className="flex flex-col gap-1">
              {build.runes.slice(0, ROWS).map((option, index) => (
                <RuneRow
                  key={index}
                  option={option}
                  index={index}
                  championId={championId}
                  canApply={context !== "game"}
                  actions={actions}
                />
              ))}
            </ul>
          )}
          {shown === "spells" && (
            <ul className="flex flex-col gap-1">
              {build.spells.slice(0, ROWS).map((option, index) => (
                <SpellRow
                  key={index}
                  option={option}
                  index={index}
                  canApply={context === "champSelect"}
                  actions={actions}
                />
              ))}
            </ul>
          )}
          {shown === "skills" && (
            <ul className="flex flex-col gap-1">
              {build.skillOrders.slice(0, 4).map((order, index) => (
                <SkillRow key={index} order={order} arena={build.mode === "arena"} />
              ))}
            </ul>
          )}
          {shown === "matchups" && (
            <div className="flex flex-col gap-2">
              <div className="flex flex-col gap-3 @[640px]:flex-row">
                <MatchupList title={t("loadout.good")} matchups={build.matchups.good} />
                <MatchupList title={t("loadout.bad")} matchups={build.matchups.bad} />
              </div>
              <p className="text-[11.5px] text-fg-subtle">{t("loadout.matchupHint")}</p>
            </div>
          )}
          {shown === "augments" && <AugmentsTab build={build} source={source} />}
          {build.dropped > 0 && (
            <p className="mt-3 text-[11px] text-fg-subtle">
              {t("loadout.dropped", { n: build.dropped })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

/** Where the numbers come from, said in the header. */
function SourceLine({ build }: { build: Build | null }) {
  const t = useT();
  const language = useLanguage();
  if (!build) return null;
  const source = t(SOURCE_LABEL[build.source]);
  return (
    <span className="flex flex-wrap items-center gap-2 text-[11.5px] text-fg-muted">
      <span>
        {build.patch
          ? t("loadout.sourcePatch", { source, patch: build.patch })
          : t("loadout.source", { source })}
      </span>
      {build.sample !== null && (
        <span className="mono text-fg-subtle">
          {t("loadout.sample", { n: build.sample.toLocaleString(language) })}
        </span>
      )}
      {build.tier !== null && (
        <Badge title={t("loadout.tierHint", { source })}>
          <span className="mono">T{build.tier}</span>
        </Badge>
      )}
    </span>
  );
}

/** The build panel for a champion, or `toolbar` and a hint while there is none. Hidden while the
 *  user has switched builds off. */
export function BuildPanel({
  championId,
  mode,
  lane,
  context,
  toolbar,
  className,
}: {
  championId: number | null;
  mode: Mode;
  lane: Position | null;
  context: BuildContext;
  toolbar?: ReactNode;
  className?: string;
}) {
  const t = useT();
  const catalog = useCatalog();
  const enabled = useSettings().builds.enabled;
  const [build, setBuild] = useState<Build | null>(null);
  if (!enabled) return null;
  const champion = championId ? catalog?.champions.get(championId) : undefined;
  const ready = championId !== null && championId > 0;
  return (
    <Panel
      eyebrow={context === "lookup" ? t("loadout.lookup") : t("loadout.panel")}
      title={champion ? `${champion.name} · ${champion.shortName}` : undefined}
      right={ready && hasNumbers(mode) ? <SourceLine build={build} /> : undefined}
      className={cx("@container", className)}
    >
      {toolbar && <div className="mb-3 flex flex-wrap items-center gap-2">{toolbar}</div>}
      {!ready ? (
        <p className="text-[12px] text-fg-subtle">
          {t(context === "lookup" ? "loadout.lookupHint" : "loadout.pickHint")}
        </p>
      ) : !hasNumbers(mode) ? (
        <EmptyState compact icon={Swords} title={t("loadout.noData")} />
      ) : (
        <BuildBody
          key={`${championId}:${mode}:${lane ?? ""}`}
          championId={championId}
          mode={mode}
          lane={lane}
          context={context}
          onBuild={setBuild}
        />
      )}
    </Panel>
  );
}

/** A champion from the catalog, by name, title or alias. */
function ChampionChooser({
  value,
  onChange,
}: {
  value: number | null;
  onChange: (id: number) => void;
}) {
  const t = useT();
  const catalog = useCatalog();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const anchor = useRef<HTMLDivElement>(null);
  const options = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (catalog?.championList ?? []).filter(
      (champion) =>
        !needle ||
        [champion.name, champion.shortName, champion.alias].some((text) =>
          text.toLowerCase().includes(needle),
        ),
    );
  }, [catalog, query]);
  const chosen = value ? catalog?.champions.get(value) : undefined;
  return (
    <div ref={anchor}>
      <Button
        size="sm"
        disabled={!catalog}
        title={catalog ? undefined : t("auto.needCatalog")}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((shown) => !shown)}
      >
        {chosen ? <ChampionIcon id={chosen.id} size={18} /> : <Search size={13} strokeWidth={2} />}
        {chosen ? chosen.shortName : t("loadout.choose")}
      </Button>
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="bottom-start"
        className="w-[340px] p-2"
        label={t("loadout.choose")}
      >
        <Input
          icon={Search}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("auto.search")}
          aria-label={t("auto.search")}
          className="w-full"
          autoFocus
        />
        <ul className="mt-2 grid max-h-[260px] grid-cols-6 gap-1 overflow-y-auto pr-1">
          {options.map((champion) => (
            <li key={champion.id}>
              <button
                type="button"
                title={`${champion.name} · ${champion.shortName}`}
                onClick={() => {
                  onChange(champion.id);
                  setQuery("");
                  setOpen(false);
                }}
                className="flex w-full flex-col items-center gap-1 rounded-6 p-1 hover-wash"
              >
                <ChampionIcon id={champion.id} size={36} />
                <span className="w-full truncate text-center text-[10.5px] text-fg-muted">
                  {champion.shortName}
                </span>
              </button>
            </li>
          ))}
        </ul>
        {options.length === 0 && (
          <p className="py-6 text-center text-[12px] text-fg-subtle">{t("auto.noMatch")}</p>
        )}
      </Popover>
    </div>
  );
}

/** The build panel without a game: a champion and a mode chosen by hand. */
export function BuildLookup() {
  const t = useT();
  const enabled = useSettings().builds.enabled;
  const [championId, setChampionId] = useState<number | null>(null);
  const [mode, setMode] = useState<Mode>("ranked");
  if (!enabled) return null;
  return (
    <BuildPanel
      championId={championId}
      mode={mode}
      lane={null}
      context="lookup"
      toolbar={
        <>
          <ChampionChooser value={championId} onChange={setChampionId} />
          <Segmented<Mode>
            size="sm"
            label={t("loadout.mode")}
            value={mode}
            options={LOOKUP_MODES.map((value) => ({
              value,
              label: value === "ranked" ? t("loadout.rift") : t(MODE_LABEL[value]),
            }))}
            onChange={setMode}
          />
        </>
      }
    />
  );
}

/** The kind of game a queue is, once the catalog names it. */
function useQueueMode(queueId: number): Mode | null {
  const queue = useCatalog()?.queues.get(queueId);
  return queue ? modeOf(queue.gameMode, queue.ranked) : null;
}

/** The panel for the champion the local player hovers or has locked, in their lane. */
export function ChampSelectBuild({ view }: { view: ChampSelectView }) {
  const mode = useQueueMode(view.queueId);
  const me = view.myTeam.find((seat) => seat.isSelf);
  if (!me || !mode) return null;
  return (
    <BuildPanel
      championId={me.championId > 0 ? me.championId : null}
      mode={mode}
      lane={me.position}
      context="champSelect"
    />
  );
}

/** The panel for the champion being played. */
export function GameBuild({ view }: { view: GameView }) {
  const mode = useQueueMode(view.queueId);
  const me = view.teams.flat().find((seat) => seat.isSelf);
  if (!me || !mode || me.championId <= 0) return null;
  return <BuildPanel championId={me.championId} mode={mode} lane={me.position} context="game" />;
}
