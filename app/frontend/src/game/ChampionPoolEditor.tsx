// Ordered champion lists, edited in place: per position for pick and ban, one list for the bench.
import type { ChampionInfo, ChampionPool, Position } from "@winer/shared";
import { positionLabel } from "@winer/shared";
import { ArrowUp, Plus, Search, X } from "lucide-react";
import { type ReactNode, useMemo, useRef, useState } from "react";

import { useLanguage, useT } from "../lib/i18n";
import { useCatalog } from "../lib/store";
import { Button, Input, Popover, Segmented } from "../ui";
import { ChampionIcon } from "./icons";

const LIMIT = 10;
type Slot = "any" | Position;
const SLOTS: Slot[] = ["any", "top", "jungle", "middle", "bottom", "utility"];

function matchesQuery(champion: ChampionInfo, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [champion.name, champion.shortName, champion.alias].some((text) =>
    text.toLowerCase().includes(needle),
  );
}

function ChampionPicker({
  exclude,
  onPick,
  disabled,
}: {
  exclude: number[];
  onPick: (id: number) => void;
  disabled: boolean;
}) {
  const t = useT();
  const catalog = useCatalog();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const anchor = useRef<HTMLDivElement>(null);
  const options = useMemo(
    () =>
      (catalog?.championList ?? []).filter(
        (champion) => !exclude.includes(champion.id) && matchesQuery(champion, query),
      ),
    [catalog, exclude, query],
  );

  return (
    <div ref={anchor} className="relative">
      <Button
        size="sm"
        icon={Plus}
        disabled={disabled || !catalog}
        title={catalog ? undefined : t("auto.needCatalog")}
        onClick={() => setOpen((value) => !value)}
      >
        {t("auto.add")}
      </Button>
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="bottom-end"
        className="w-[340px] p-2"
        label={t("auto.add")}
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
                  onPick(champion.id);
                  setQuery("");
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

/** One ordered list of champions: add from the catalog, move up, remove. `leading` sits at the
 *  start of the toolbar, where the pool editor puts its position switch. */
export function ChampionList({
  list,
  onChange,
  limit = LIMIT,
  hint,
  leading,
}: {
  list: number[];
  onChange: (list: number[]) => void;
  limit?: number;
  hint: string;
  leading?: ReactNode;
}) {
  const t = useT();
  const catalog = useCatalog();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        {leading ?? <span />}
        <span className="flex items-center gap-2">
          <span className="mono text-[11px] text-fg-subtle">{t("auto.full", { n: limit })}</span>
          <ChampionPicker
            exclude={list}
            disabled={list.length >= limit}
            onPick={(id) => onChange([...list, id])}
          />
        </span>
      </div>
      {list.length === 0 ? (
        <p className="rounded-6 border border-dashed border-border-strong px-3 py-4 text-center text-[12px] text-fg-subtle">
          {t("auto.poolEmpty")}
        </p>
      ) : (
        <ol className="flex flex-wrap gap-2">
          {list.map((id, index) => {
            const name = catalog?.champions.get(id)?.name ?? `#${id}`;
            return (
              <li
                key={id}
                className="flex items-center gap-2 rounded-6 bg-inset py-1 pr-1 pl-1 hairline"
              >
                <span className="mono w-4 text-center text-[11px] text-fg-subtle">{index + 1}</span>
                <ChampionIcon id={id} size={26} />
                <span className="max-w-[96px] truncate text-[12.5px] text-fg">{name}</span>
                <span className="flex">
                  {index > 0 && (
                    <button
                      type="button"
                      aria-label={t("auto.moveUp", { name })}
                      title={t("auto.moveUp", { name })}
                      onClick={() =>
                        onChange(
                          list.map((value, at) =>
                            at === index - 1
                              ? id
                              : at === index
                                ? (list[index - 1] ?? value)
                                : value,
                          ),
                        )
                      }
                      className="grid size-6 place-items-center rounded-4 text-fg-subtle hover:text-fg hover-wash"
                    >
                      <ArrowUp size={12} strokeWidth={2} />
                    </button>
                  )}
                  <button
                    type="button"
                    aria-label={t("auto.remove", { name })}
                    title={t("auto.remove", { name })}
                    onClick={() => onChange(list.filter((value) => value !== id))}
                    className="grid size-6 place-items-center rounded-4 text-fg-subtle hover:text-danger hover-wash"
                  >
                    <X size={12} strokeWidth={2} />
                  </button>
                </span>
              </li>
            );
          })}
        </ol>
      )}
      <p className="text-[11.5px] text-fg-subtle">{hint}</p>
    </div>
  );
}

/** Ordered lists per position, with `any` as the fallback. */
export function ChampionPoolEditor({
  pool,
  onChange,
  label,
}: {
  pool: ChampionPool;
  onChange: (pool: ChampionPool) => void;
  label: string;
}) {
  const t = useT();
  const language = useLanguage();
  const [slot, setSlot] = useState<Slot>("any");
  const slotLabel = (value: Slot) =>
    value === "any" ? t("auto.poolAny") : positionLabel(value, language);
  return (
    <ChampionList
      list={pool[slot]}
      onChange={(next) => onChange({ ...pool, [slot]: next })}
      hint={t("auto.poolHint")}
      leading={
        <Segmented
          size="sm"
          label={label}
          value={slot}
          onChange={setSlot}
          options={SLOTS.map((value) => ({
            value,
            label:
              pool[value].length > 0
                ? `${slotLabel(value)} ${pool[value].length}`
                : slotLabel(value),
          }))}
        />
      }
    />
  );
}
