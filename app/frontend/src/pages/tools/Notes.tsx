// Tools: every note the user wrote on other players, newest first, searchable by name or text.
// A name opens the player's history, where the note is edited; a note can be deleted here.
import { riotId } from "@winer/shared";
import { Search, Trash2 } from "lucide-react";
import { useState } from "react";

import { NoteChip } from "../../game/notes";
import { errorMessage } from "../../lib/backend";
import { useLanguage, useT } from "../../lib/i18n";
import { useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import { useShell } from "../../shell/navigation";
import { EmptyState, IconButton, Input, Panel, toast } from "../../ui";

export function NotesPanel() {
  const t = useT();
  const language = useLanguage();
  const store = useStore();
  const { navigate } = useShell();
  const notes = useAsync(() => store.backend.call("list_player_notes"), []);
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const all = notes.data ?? [];
  const shown = needle
    ? all.filter(
        ({ note }) =>
          riotId(note.name).toLowerCase().includes(needle) ||
          note.text.toLowerCase().includes(needle),
      )
    : all;
  const day = new Intl.DateTimeFormat(language, { dateStyle: "medium" });

  const remove = async (puuid: string) => {
    try {
      await store.backend.call("delete_player_note", { puuid });
      notes.reload();
      toast(t("note.removed"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    }
  };

  return (
    <Panel
      eyebrow={t("note.list")}
      className="col-span-12"
      right={
        all.length > 0 ? (
          <Input
            icon={Search}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("note.search")}
            aria-label={t("note.search")}
            className="w-56"
          />
        ) : undefined
      }
    >
      <p className="mb-2 text-[12px] leading-4 text-fg-muted">{t("note.listHint")}</p>
      {all.length === 0 ? (
        <EmptyState compact title={t("note.empty")} />
      ) : shown.length === 0 ? (
        <EmptyState compact title={t("note.noMatch")} />
      ) : (
        <ul aria-label={t("note.list")} className="flex flex-col">
          {shown.map(({ puuid, note }) => {
            const name = riotId(note.name) || t("note.unknown");
            return (
              <li
                key={puuid}
                className="flex min-w-0 items-center gap-3 border-b border-border py-2 last:border-b-0"
              >
                <button
                  type="button"
                  onClick={() => navigate({ page: "history", puuid })}
                  className="min-w-0 shrink-0 truncate rounded-4 text-left text-[13px] font-medium text-fg hover:underline"
                >
                  {name}
                </button>
                <NoteChip note={note} />
                <span
                  className="min-w-0 flex-1 truncate text-[12.5px] text-fg-muted"
                  title={note.text}
                >
                  {note.text}
                </span>
                <span className="mono shrink-0 text-[11px] text-fg-subtle">
                  {day.format(new Date(note.updatedAt))}
                </span>
                <IconButton
                  icon={Trash2}
                  label={t("note.deleteOf", { name })}
                  onClick={() => void remove(puuid)}
                />
              </li>
            );
          })}
        </ul>
      )}
    </Panel>
  );
}
