// The user's notes on other players: the chip that marks a player wherever they appear, and the
// editor on their history page. Notes stay on this PC and never go into a chat message.
import type { NoteTag, PlayerNote, RiotId } from "@winer/shared";
import { StickyNote, Trash2 } from "lucide-react";
import { useEffect, useId, useState } from "react";

import { errorMessage } from "../lib/backend";
import { useT } from "../lib/i18n";
import { useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import { Badge, Button, Card, Segmented, Skeleton, type Tone, toast } from "../ui";

/** Characters in a note at most, as the core counts them (`notes::MAX_TEXT`). */
export const NOTE_LIMIT = 200;

export const NOTE_TAGS: readonly NoteTag[] = ["reliable", "weak", "toxic", "troll"];

const TONE: Record<NoteTag, Tone> = {
  reliable: "ok",
  weak: "warning",
  toxic: "danger",
  troll: "danger",
};

/** `▤ 坑`: the tag, or 备注 for a note of text only, with the text on hover. */
export function NoteChip({ note }: { note: PlayerNote }) {
  const t = useT();
  const label = note.tag ? t(`note.tag.${note.tag}`) : t("note.chip");
  return (
    <Badge tone={note.tag ? TONE[note.tag] : "neutral"} title={note.text || label}>
      <StickyNote size={11} strokeWidth={2} aria-hidden />
      {label}
    </Badge>
  );
}

/** The note on one player, on their history page: a tag, a line of text, save and delete. */
export function NoteEditor({ puuid, name }: { puuid: string; name: RiotId | null }) {
  const t = useT();
  const store = useStore();
  const field = useId();
  const saved = useAsync(() => store.backend.call("get_player_note", { puuid }), [puuid]);
  const [tag, setTag] = useState<NoteTag | null>(null);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    setTag(saved.data?.tag ?? null);
    setText(saved.data?.text ?? "");
  }, [saved.data]);
  const length = Array.from(text.trim()).length;
  const changed = tag !== (saved.data?.tag ?? null) || text.trim() !== (saved.data?.text ?? "");

  const save = async () => {
    setBusy(true);
    try {
      const note = await store.backend.call("set_player_note", { puuid, tag, text, name });
      saved.reload();
      toast(note ? t("note.saved") : t("note.removed"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    setBusy(true);
    try {
      await store.backend.call("delete_player_note", { puuid });
      saved.reload();
      toast(t("note.removed"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };

  // The form waits for the saved note: one arriving late would overwrite what the user typed.
  if (saved.data === undefined && !saved.error) {
    return (
      <Card>
        <Skeleton className="h-[92px] w-full" />
      </Card>
    );
  }
  return (
    <Card className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="eyebrow">{t("note.title")}</h2>
        <Segmented<NoteTag | "none">
          size="sm"
          label={t("note.tag")}
          value={tag ?? "none"}
          options={[
            { value: "none", label: t("note.tag.none") },
            ...NOTE_TAGS.map((value) => ({ value, label: t(`note.tag.${value}`) })),
          ]}
          onChange={(value) => setTag(value === "none" ? null : value)}
        />
      </div>
      <label htmlFor={field} className="sr-only">
        {t("note.text")}
      </label>
      <textarea
        id={field}
        value={text}
        rows={2}
        maxLength={NOTE_LIMIT * 2}
        placeholder={t("note.placeholder")}
        aria-invalid={length > NOTE_LIMIT || undefined}
        onChange={(event) => setText(event.target.value)}
        className="w-full resize-y rounded-6 bg-surface px-2.5 py-2 text-[13px] leading-5 text-fg outline-none hairline placeholder:text-fg-subtle hover:border-border-strong focus:border-accent"
      />
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-[11.5px] text-fg-subtle">{t("note.privacy")}</span>
        <span className="flex items-center gap-2">
          <span
            className={
              length > NOTE_LIMIT
                ? "mono text-[11px] text-danger"
                : "mono text-[11px] text-fg-subtle"
            }
          >
            {length}/{NOTE_LIMIT}
          </span>
          {saved.data && (
            <Button
              size="sm"
              variant="ghost"
              icon={Trash2}
              disabled={busy}
              onClick={() => void remove()}
            >
              {t("note.delete")}
            </Button>
          )}
          <Button
            size="sm"
            variant="accent"
            loading={busy}
            disabled={!changed || length > NOTE_LIMIT}
            onClick={() => void save()}
          >
            {t("note.save")}
          </Button>
        </span>
      </div>
    </Card>
  );
}
