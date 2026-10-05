// The profile background: the skin behind the player's profile, chosen from every skin there is.
import { ChevronDown, Search, X } from "lucide-react";
import { useMemo, useRef, useState } from "react";

import { ChampionIcon } from "../../game/icons";
import { errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { useT } from "../../lib/i18n";
import { useCatalog, useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import {
  Badge,
  Button,
  ErrorNote,
  IconButton,
  Input,
  Pager,
  Panel,
  Popover,
  Skeleton,
  Toggle,
  toast,
} from "../../ui";
import { Art } from "./Art";
import { SKIN_PAGE_SIZE, filterSkins } from "./profile";

/** Narrows the picker to one champion, chosen from the catalog by name. */
function ChampionFilter({
  value,
  onChange,
}: {
  value: number | null;
  onChange: (id: number | null) => void;
}) {
  const t = useT();
  const catalog = useCatalog();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const anchor = useRef<HTMLDivElement>(null);
  const chosen = value === null ? undefined : catalog?.champions.get(value);
  const options = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (catalog?.championList ?? []).filter(
      (champion) =>
        !needle ||
        [champion.name, champion.shortName, champion.alias].some((name) =>
          name.toLowerCase().includes(needle),
        ),
    );
  }, [catalog, query]);
  const pick = (id: number | null) => {
    onChange(id);
    setOpen(false);
    setQuery("");
  };
  return (
    <div ref={anchor} className="flex items-center gap-0.5">
      <Button
        size="sm"
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={!catalog}
        title={catalog ? t("profile.background.champion") : t("auto.needCatalog")}
        onClick={() => setOpen((value) => !value)}
      >
        {chosen && <ChampionIcon id={chosen.id} size={18} />}
        {chosen?.shortName ?? t("profile.background.allChampions")}
        <ChevronDown size={13} strokeWidth={2} aria-hidden />
      </Button>
      {chosen && (
        <IconButton
          icon={X}
          size={24}
          label={t("profile.background.allChampions")}
          onClick={() => pick(null)}
        />
      )}
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        className="w-[340px] p-2"
        label={t("profile.background.champion")}
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
                aria-pressed={champion.id === value}
                onClick={() => pick(champion.id)}
                className={cx(
                  "flex w-full flex-col items-center gap-1 rounded-6 p-1 hover-wash",
                  champion.id === value && "bg-accent-soft",
                )}
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

export function BackgroundPanel() {
  const t = useT();
  const store = useStore();
  const catalog = useCatalog();
  const skins = useAsync(() => store.backend.call("get_skins"), []);
  const background = useAsync(() => store.backend.call("get_profile_background"), []);
  /** What the client reported after the last change made here; until then, what it said first. */
  const [reported, setReported] = useState<number | null | undefined>(undefined);
  const current = reported !== undefined ? reported : (background.data ?? null);
  const [query, setQuery] = useState("");
  const [champion, setChampion] = useState<number | null>(null);
  const [ownedOnly, setOwnedOnly] = useState(false);
  const [page, setPage] = useState(1);
  const [chosen, setChosen] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  /** The skin the client did not take, said until the next try. */
  const [refused, setRefused] = useState<number | null>(null);

  const byId = useMemo(
    () => new Map((skins.data ?? []).map((skin) => [skin.id, skin])),
    [skins.data],
  );
  const filtered = useMemo(
    () => filterSkins(skins.data ?? [], catalog?.champions, { query, champion, ownedOnly }),
    [skins.data, catalog, query, champion, ownedOnly],
  );
  const pages = Math.max(1, Math.ceil(filtered.length / SKIN_PAGE_SIZE));
  const at = Math.min(page, pages);
  const visible = filtered.slice((at - 1) * SKIN_PAGE_SIZE, at * SKIN_PAGE_SIZE);
  const name = (id: number | null) =>
    id === null ? t("profile.background.none") : (byId.get(id)?.name ?? `#${id}`);
  const currentSkin = current === null ? undefined : byId.get(current);
  const chosenSkin = chosen === null ? undefined : byId.get(chosen);
  /** A narrower or wider list starts on its first page. */
  const filter = (change: () => void) => {
    change();
    setPage(1);
  };

  const apply = async () => {
    if (chosen === null) return;
    setSaving(true);
    try {
      const shown = await store.backend.call("set_profile_background", { skinId: chosen });
      setReported(shown);
      if (shown === chosen) {
        setRefused(null);
        toast(t("profile.background.done", { name: name(chosen) }), "ok");
      } else {
        setRefused(chosen);
      }
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Panel
      eyebrow={t("profile.background.title")}
      className="col-span-12"
      right={
        skins.data && (
          <span className="mono text-[11px] text-fg-subtle">
            {t("profile.background.count", { n: filtered.length })}
          </span>
        )
      }
    >
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-start gap-4">
          <Art
            path={currentSkin?.splash}
            alt={currentSkin?.name ?? ""}
            className="aspect-video w-[240px] rounded-6 hairline"
          />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="eyebrow">{t("profile.background.current")}</span>
            {background.loading && reported === undefined ? (
              <Skeleton className="h-5 w-40" />
            ) : (
              <span className="text-[16px] font-medium text-fg">{name(current)}</span>
            )}
            <p className="text-[12px] leading-4 text-fg-muted">{t("profile.background.hint")}</p>
            {refused !== null && (
              <p role="alert" className="text-[12px] leading-4 text-warning">
                {t("profile.background.refused", { name: name(refused), current: name(current) })}
              </p>
            )}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Input
            icon={Search}
            value={query}
            onChange={(event) => {
              const next = event.target.value;
              filter(() => setQuery(next));
            }}
            placeholder={t("profile.background.search")}
            aria-label={t("profile.background.search")}
            className="w-[240px]"
          />
          <ChampionFilter value={champion} onChange={(id) => filter(() => setChampion(id))} />
          <label className="ml-auto flex items-center gap-2 text-[12.5px] text-fg-muted">
            <Toggle
              checked={ownedOnly}
              onChange={(next) => filter(() => setOwnedOnly(next))}
              label={t("profile.background.owned")}
            />
            {t("profile.background.owned")}
          </label>
        </div>

        {skins.error !== undefined && !skins.data ? (
          <ErrorNote
            title={t("profile.background.loadFailed")}
            detail={errorMessage(skins.error)}
            retryLabel={t("common.retry")}
            onRetry={skins.reload}
          />
        ) : !skins.data ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-2">
            {Array.from({ length: 12 }, (_, index) => (
              <Skeleton key={index} className="aspect-square w-full" />
            ))}
          </div>
        ) : visible.length === 0 ? (
          <p className="rounded-6 border border-dashed border-border-strong px-3 py-6 text-center text-[12px] text-fg-subtle">
            {t("profile.background.empty")}
          </p>
        ) : (
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-2">
            {visible.map((skin) => {
              const selected = skin.id === chosen;
              const label = skin.owned
                ? skin.name
                : `${skin.name} · ${t("profile.background.unowned")}`;
              return (
                <li key={skin.id}>
                  <button
                    type="button"
                    aria-pressed={selected}
                    aria-label={label}
                    title={label}
                    onClick={() => setChosen(skin.id)}
                    className={cx(
                      "flex w-full flex-col gap-1 rounded-6 p-1 text-left transition-colors duration-150 hover-wash",
                      selected && "bg-accent-soft",
                    )}
                  >
                    <span className="relative block">
                      <Art
                        path={skin.tile}
                        alt=""
                        className={cx(
                          "aspect-square w-full rounded-4",
                          !skin.owned && "opacity-60",
                        )}
                      />
                      {skin.id === current && (
                        <Badge tone="accent" className="absolute top-1 left-1">
                          {t("profile.background.current")}
                        </Badge>
                      )}
                      {!skin.owned && (
                        <Badge className="absolute right-1 bottom-1">
                          {t("profile.background.unowned")}
                        </Badge>
                      )}
                      {selected && (
                        <span
                          aria-hidden
                          className="absolute inset-0 rounded-4 ring-2 ring-accent"
                        />
                      )}
                    </span>
                    <span className="truncate text-[11.5px] text-fg">{skin.name}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}

        <div className="flex flex-wrap items-center justify-between gap-3">
          {pages > 1 ? (
            <Pager
              page={at}
              known={pages}
              more={false}
              onPage={setPage}
              labels={{
                nav: t("profile.background.pages"),
                previous: t("history.previous"),
                next: t("history.next"),
                page: (n) => t("history.page", { n }),
              }}
            />
          ) : (
            <span />
          )}
          <span className="flex items-center gap-3">
            <span className="text-[12px] text-fg-muted">
              {chosenSkin
                ? t("profile.background.chosen", { name: chosenSkin.name })
                : t("profile.background.pick")}
            </span>
            <Button
              variant="accent"
              size="sm"
              loading={saving}
              disabled={chosen === null || chosen === current}
              onClick={() => void apply()}
            >
              {t("profile.background.apply")}
            </Button>
          </span>
        </div>
      </div>
    </Panel>
  );
}
