// The challenge tokens and the title on the player's profile: three slots, left to right, each
// any challenge with a level, and one title from the ones the player has.
import type { ChallengeProfile, ChallengeToken, Language, TitleChoice } from "@winer/shared";
import { tierLabel } from "@winer/shared";
import { ChevronDown, Plus, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { useLanguage, useT } from "../../lib/i18n";
import { useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import {
  Button,
  ErrorNote,
  IconButton,
  Input,
  Panel,
  Popover,
  Row,
  Skeleton,
  toast,
} from "../../ui";
import { Art } from "./Art";

const SLOTS = [0, 1, 2] as const;
/** Challenge tokens are drawn for the game's dark ground. */
const TOKEN_GROUND = "bg-glyph-plate text-glyph-ink";

function levelOf(token: ChallengeToken, language: Language): string {
  return token.level ? tierLabel(token.level, language, true) : "";
}

/** One slot: the token in it, a picker of every other token, and a way to empty it. */
function TokenSlot({
  index,
  token,
  choices,
  taken,
  onChange,
}: {
  index: number;
  token: ChallengeToken | undefined;
  choices: readonly ChallengeToken[];
  /** In the other slots: a token shows once. */
  taken: readonly number[];
  onChange: (id: number | null) => void;
}) {
  const t = useT();
  const language = useLanguage();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const anchor = useRef<HTMLDivElement>(null);
  const n = index + 1;
  const options = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return choices.filter(
      (choice) =>
        !taken.includes(choice.id) && (!needle || choice.name.toLowerCase().includes(needle)),
    );
  }, [choices, taken, query]);
  const name = token ? token.name || `#${token.id}` : t("profile.challenges.choose");
  return (
    <div
      ref={anchor}
      className="relative flex min-w-0 flex-col items-center rounded-6 bg-inset p-1 hairline"
    >
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`${t("profile.challenges.slot", { n })}: ${name}`}
        title={token?.description || undefined}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full flex-col items-center gap-1.5 rounded-6 px-1 pt-2 pb-1.5 hover-wash"
      >
        {token ? (
          <Art path={token.icon} alt="" ground={TOKEN_GROUND} className="size-12 rounded-full" />
        ) : (
          <span className="grid size-12 place-items-center rounded-full border border-dashed border-border-strong text-fg-subtle">
            <Plus size={16} strokeWidth={2} aria-hidden />
          </span>
        )}
        <span className="w-full truncate text-center text-[12px] text-fg">{name}</span>
        <span className="text-[11px] text-fg-subtle">
          {token
            ? levelOf(token, language) || t("profile.challenges.slot", { n })
            : t("profile.challenges.slot", { n })}
        </span>
      </button>
      {token && (
        <IconButton
          icon={X}
          size={24}
          label={t("profile.challenges.clearSlot", { n })}
          onClick={() => onChange(null)}
          className="absolute top-1 right-1"
        />
      )}
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        className="w-[320px] p-2"
        label={t("profile.challenges.slot", { n })}
      >
        <Input
          icon={Search}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("profile.challenges.search")}
          aria-label={t("profile.challenges.search")}
          className="w-full"
          autoFocus
        />
        <ul className="mt-2 flex max-h-[280px] flex-col gap-0.5 overflow-y-auto pr-1">
          {options.map((choice) => (
            <li key={choice.id}>
              <button
                type="button"
                aria-pressed={choice.id === token?.id}
                title={choice.description || undefined}
                onClick={() => {
                  onChange(choice.id);
                  setOpen(false);
                  setQuery("");
                }}
                className={cx(
                  "flex w-full items-center gap-2.5 rounded-6 px-1.5 py-1 text-left hover-wash",
                  choice.id === token?.id && "bg-accent-soft",
                )}
              >
                <Art
                  path={choice.icon}
                  alt=""
                  ground={TOKEN_GROUND}
                  className="size-7 rounded-full"
                />
                <span className="min-w-0 flex-1 truncate text-[12.5px] text-fg">{choice.name}</span>
                <span className="text-[11px] text-fg-subtle">{levelOf(choice, language)}</span>
              </button>
            </li>
          ))}
        </ul>
        {options.length === 0 && (
          <p className="py-6 text-center text-[12px] text-fg-subtle">
            {choices.length === 0 ? t("profile.challenges.none") : t("profile.challenges.noMatch")}
          </p>
        )}
      </Popover>
    </div>
  );
}

function TitlePicker({
  titles,
  value,
  onChange,
}: {
  titles: readonly TitleChoice[];
  value: number | null;
  onChange: (id: number) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const chosen = titles.find((title) => title.id === value);
  return (
    <div ref={anchor}>
      <Button
        size="sm"
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={titles.length === 0}
        onClick={() => setOpen((value) => !value)}
      >
        {chosen?.name ?? t("profile.challenges.noTitle")}
        <ChevronDown size={13} strokeWidth={2} aria-hidden />
      </Button>
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="bottom-end"
        className="w-[240px] p-1"
        label={t("profile.challenges.chooseTitle")}
      >
        <ul className="flex max-h-[280px] flex-col gap-0.5 overflow-y-auto">
          {titles.map((title) => (
            <li key={title.id}>
              <button
                type="button"
                aria-pressed={title.id === value}
                onClick={() => {
                  onChange(title.id);
                  setOpen(false);
                }}
                className={cx(
                  "flex h-8 w-full items-center rounded-6 px-2.5 text-left text-[12.5px] text-fg hover-wash",
                  title.id === value && "bg-accent-soft font-medium",
                )}
              >
                {title.name}
              </button>
            </li>
          ))}
        </ul>
      </Popover>
    </div>
  );
}

/** Whether the client shows the tokens and the title asked for. */
function shows(
  profile: ChallengeProfile,
  tokens: readonly number[],
  title: number | null,
): boolean {
  const shown = profile.tokens.map((token) => token.id);
  return (
    shown.length === tokens.length &&
    shown.every((id, index) => id === tokens[index]) &&
    (title === null || profile.title?.id === title)
  );
}

export function ChallengePanel() {
  const t = useT();
  const store = useStore();
  const profile = useAsync(() => store.backend.call("get_challenge_profile"), []);
  /** What the client reports, from the first read or the last change. */
  const [shown, setShown] = useState<ChallengeProfile | null>(null);
  const [slots, setSlots] = useState<(number | null)[]>([null, null, null]);
  const [title, setTitle] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [partly, setPartly] = useState(false);

  const take = (data: ChallengeProfile) => {
    setShown(data);
    setSlots(SLOTS.map((index) => data.tokens[index]?.id ?? null));
    setTitle(data.title?.id ?? null);
  };
  useEffect(() => {
    if (profile.data) take(profile.data);
  }, [profile.data]);

  const known = useMemo(() => {
    const map = new Map<number, ChallengeToken>();
    for (const token of [...(shown?.challenges ?? []), ...(shown?.tokens ?? [])]) {
      if (!map.has(token.id)) map.set(token.id, token);
    }
    return map;
  }, [shown]);
  const chosen = slots.filter((id): id is number => id !== null);
  const changed =
    shown !== null &&
    (chosen.join() !== shown.tokens.map((token) => token.id).join() ||
      title !== (shown.title?.id ?? null));

  const apply = async () => {
    setSaving(true);
    try {
      const result = await store.backend.call("set_challenge_profile", {
        challengeIds: chosen,
        titleId: title,
      });
      const took = shows(result, chosen, title);
      take(result);
      setPartly(!took);
      if (took) toast(t("profile.challenges.done"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Panel eyebrow={t("profile.challenges.title")} className="col-span-12 @[900px]:col-span-6">
      {profile.error !== undefined && !shown ? (
        <ErrorNote
          title={t("profile.challenges.loadFailed")}
          detail={errorMessage(profile.error)}
          retryLabel={t("common.retry")}
          onRetry={profile.reload}
        />
      ) : !shown ? (
        <div className="grid grid-cols-3 gap-2">
          {SLOTS.map((index) => (
            <Skeleton key={index} className="h-[104px] w-full" />
          ))}
        </div>
      ) : (
        <div className="flex flex-col">
          <p className="pb-3 text-[12px] leading-4 text-fg-muted">{t("profile.challenges.hint")}</p>
          <div className="grid grid-cols-3 gap-2">
            {SLOTS.map((index) => {
              const id = slots[index] ?? null;
              return (
                <TokenSlot
                  key={index}
                  index={index}
                  token={id === null ? undefined : known.get(id)}
                  choices={shown.challenges}
                  taken={slots.filter(
                    (other, at): other is number => at !== index && other !== null,
                  )}
                  onChange={(next) =>
                    setSlots((current) => current.map((value, at) => (at === index ? next : value)))
                  }
                />
              );
            })}
          </div>
          <Row label={t("profile.challenges.titleLabel")} className="mt-1">
            <TitlePicker titles={shown.titles} value={title} onChange={setTitle} />
          </Row>
          {partly && (
            <p role="alert" className="pb-2 text-[12px] leading-4 text-warning">
              {t("profile.challenges.partly")}
            </p>
          )}
          <div className="flex items-center justify-end gap-2 pt-1">
            <Button
              size="sm"
              variant="ghost"
              disabled={chosen.length === 0}
              onClick={() => setSlots([null, null, null])}
            >
              {t("profile.challenges.clear")}
            </Button>
            <Button size="sm" variant="ghost" disabled={!changed} onClick={() => take(shown)}>
              {t("profile.challenges.reset")}
            </Button>
            <Button
              size="sm"
              variant="accent"
              loading={saving}
              disabled={!changed}
              onClick={() => void apply()}
            >
              {t("profile.challenges.apply")}
            </Button>
          </div>
        </div>
      )}
    </Panel>
  );
}
