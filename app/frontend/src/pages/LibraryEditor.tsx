// The downloaded library's editor (在线语录 › 查看和编辑): one of its two sets at a time, searched and
// paged, each line edited in place or removed, and a field that adds one at the top. Every change
// saves both sets whole, one save after another; the core cleans them and answers what it kept.
import type { ComebackLibrary, ComebackLibraryLines, LibraryLevel } from "@winer/shared";
import { MessageSquareQuote, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { errorMessage } from "../lib/backend";
import { LINE_LENGTH } from "../lib/comeback";
import { useT } from "../lib/i18n";
import { useStore } from "../lib/store";
import {
  Button,
  CommitInput,
  Dialog,
  EmptyState,
  ErrorNote,
  IconButton,
  Input,
  Pager,
  Segmented,
  Skeleton,
  toast,
} from "../ui";

type LibrarySet = Exclude<LibraryLevel, "both">;

const SETS: readonly LibrarySet[] = ["mild", "strong"];

/** Lines a page of the list shows. */
export const LIBRARY_PAGE = 50;

/** Opens on the downloaded library; `onSaved` hears every status a save answers. */
export function LibraryEditor({
  open,
  onClose,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  onSaved: (status: ComebackLibrary) => void;
}) {
  const t = useT();
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("comeback.libraryEditTitle")}
      closeLabel={t("common.close")}
      size="lg"
    >
      {open && <Editor onSaved={onSaved} />}
    </Dialog>
  );
}

function Editor({ onSaved }: { onSaved: (status: ComebackLibrary) => void }) {
  const t = useT();
  const store = useStore();
  const [lines, setLines] = useState<ComebackLibraryLines | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [set, setSet] = useState<LibrarySet>("mild");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [draft, setDraft] = useState("");
  /** What the list shows: the newest change, before its save answers. */
  const current = useRef<ComebackLibraryLines | null>(null);
  /** The saves, one after another, and the newest one's number. */
  const saves = useRef<Promise<void>>(Promise.resolve());
  const newest = useRef(0);

  const show = (next: ComebackLibraryLines) => {
    current.current = next;
    setLines(next);
  };
  const load = async () => {
    setFailure(null);
    try {
      show(await store.backend.call("comeback_library_lines"));
    } catch (error) {
      setFailure(errorMessage(error));
    }
  };
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- read once, when the editor opens
  }, []);

  /** Applies `edit` to the set shown, at once, and saves both sets after the saves before it. */
  const change = (edit: (list: readonly string[]) => string[]) => {
    const before = current.current;
    if (!before) return;
    const next = { ...before, [set]: edit(before[set]) };
    show(next);
    const number = (newest.current += 1);
    saves.current = saves.current.then(async () => {
      try {
        const saved = await store.backend.call("edit_comeback_library", { lines: next });
        onSaved(saved.status);
        // A newer change is on its way: its own answer will show the lines as kept.
        if (number === newest.current) show(saved.lines);
      } catch (error) {
        toast(errorMessage(error), "danger");
        if (number === newest.current) await load();
      }
    });
  };

  if (failure !== null && lines === null) {
    return (
      <div className="p-5">
        <ErrorNote
          title={t("comeback.libraryLinesFailed")}
          detail={failure}
          retryLabel={t("common.retry")}
          onRetry={() => void load()}
        />
      </div>
    );
  }
  if (lines === null) {
    return (
      <div className="flex flex-col gap-2 p-5">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
      </div>
    );
  }

  const needle = query.trim();
  const matching = lines[set]
    .map((text, index) => ({ text, index }))
    .filter((entry) => needle === "" || entry.text.includes(needle));
  const pages = Math.max(1, Math.ceil(matching.length / LIBRARY_PAGE));
  const shown = Math.min(page, pages);
  const rows = matching.slice((shown - 1) * LIBRARY_PAGE, shown * LIBRARY_PAGE);
  const add = () => {
    const text = draft.trim();
    if (!text) return;
    change((list) => [text, ...list]);
    setDraft("");
    setQuery("");
    setPage(1);
  };

  return (
    <>
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-5 py-3">
        <Segmented<LibrarySet>
          size="sm"
          label={t("comeback.libraryLevel")}
          value={set}
          options={SETS.map((value) => ({
            value,
            label: t("comeback.librarySet", {
              level: t(`comeback.level.${value}`),
              n: lines[value].length,
            }),
          }))}
          onChange={(value) => {
            setSet(value);
            setPage(1);
          }}
        />
        <div className="relative min-w-[200px] flex-1">
          <Search
            size={14}
            strokeWidth={2}
            aria-hidden
            className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-fg-subtle"
          />
          <Input
            type="search"
            aria-label={t("comeback.librarySearch")}
            placeholder={t("comeback.librarySearch")}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setPage(1);
            }}
            className="w-full pl-8"
          />
        </div>
      </div>
      <p className="shrink-0 px-5 pt-2.5 text-[11.5px] leading-4 text-fg-subtle">
        {t("comeback.libraryEditHint")}
      </p>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
        className="flex shrink-0 items-center gap-2 px-5 pt-2.5"
      >
        <Input
          aria-label={t("comeback.libraryNewLine", { level: t(`comeback.level.${set}`) })}
          placeholder={t("comeback.libraryNewLinePlaceholder", {
            level: t(`comeback.level.${set}`),
          })}
          value={draft}
          maxLength={LINE_LENGTH}
          onChange={(event) => setDraft(event.target.value)}
          className="min-w-0 flex-1"
        />
        <Button type="submit" icon={Plus} disabled={draft.trim() === ""}>
          {t("list.add")}
        </Button>
      </form>
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-2">
        {rows.length === 0 ? (
          <EmptyState
            compact
            icon={MessageSquareQuote}
            title={t(needle === "" ? "comeback.librarySetEmpty" : "comeback.libraryNoMatch", {
              query: needle,
            })}
          />
        ) : (
          <ol aria-label={t(`comeback.level.${set}`)} className="flex flex-col">
            {rows.map(({ text, index }) => (
              <li
                key={`${set}-${index}`}
                className="flex items-center gap-2 border-b border-border py-1.5"
              >
                <span
                  aria-hidden
                  className="mono w-10 shrink-0 text-right text-[11px] text-fg-subtle"
                >
                  {index + 1}
                </span>
                <CommitInput
                  aria-label={t("comeback.libraryLine", { n: index + 1 })}
                  value={text}
                  maxLength={LINE_LENGTH}
                  onCommit={(next) =>
                    change((list) => list.map((old, at) => (at === index ? next : old)))
                  }
                  className="min-w-0 flex-1"
                />
                <IconButton
                  icon={Trash2}
                  tone="danger"
                  label={t("comeback.libraryRemoveLine", { n: index + 1 })}
                  onClick={() => change((list) => list.filter((_, at) => at !== index))}
                />
              </li>
            ))}
          </ol>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border px-5 py-2.5">
        <span className="text-[12px] text-fg-muted">
          {needle === ""
            ? t("comeback.libraryShown", {
                from: rows.length === 0 ? 0 : (shown - 1) * LIBRARY_PAGE + 1,
                to: (shown - 1) * LIBRARY_PAGE + rows.length,
                n: matching.length,
              })
            : t("comeback.libraryMatches", { n: matching.length })}
        </span>
        <Pager
          page={shown}
          known={pages}
          more={false}
          onPage={setPage}
          labels={{
            nav: t("comeback.libraryPages"),
            previous: t("history.previous"),
            next: t("history.next"),
            page: (n) => t("history.page", { n }),
          }}
        />
      </div>
    </>
  );
}
