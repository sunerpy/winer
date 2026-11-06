// A list of lines edited in place, shared by the Comebacks and Phrases pages.
import { Plus, Trash2 } from "lucide-react";
import { type ReactNode, useState } from "react";

import { LINE_LENGTH } from "../lib/comeback";
import { useT } from "../lib/i18n";
import { Badge, Button, CommitInput, IconButton, Input } from "../ui";

/** `n` of `max`, for a panel's header. */
export function Count({ n, max }: { n: number; max: number }) {
  const t = useT();
  return <span className="mono text-[12px] text-fg-subtle">{t("list.count", { n, max })}</span>;
}

/** Lines of text one to a row, each edited in place (saved when the field is left) or removed,
 *  and a field below that adds one; `empty` stands in for the rows while there are none. The core
 *  trims, clips and drops blanks on save. */
export function TextList({
  items,
  limit,
  listLabel,
  label,
  removeLabel,
  newLabel,
  placeholder,
  empty,
  firstMark,
  onChange,
}: {
  items: readonly string[];
  limit: number;
  listLabel: string;
  /** The accessible names of row `n`, counted from 1, and of its remove button. */
  label: (n: number) => string;
  removeLabel: (n: number) => string;
  newLabel: string;
  placeholder: string;
  empty: ReactNode;
  /** What the first row is used for, shown beside it. */
  firstMark?: string;
  /** A change to the newest list. */
  onChange: (change: (items: readonly string[]) => string[]) => void;
}) {
  const t = useT();
  const [draft, setDraft] = useState("");
  const full = items.length >= limit;
  const add = () => {
    const text = draft.trim();
    if (!text || full) return;
    onChange((list) => [...list, text]);
    setDraft("");
  };
  return (
    <>
      {items.length > 0 ? (
        <ol aria-label={listLabel} className="flex flex-col">
          {items.map((item, index) => (
            <li key={index} className="flex items-center gap-2 border-b border-border py-1.5">
              <span aria-hidden className="mono w-6 shrink-0 text-right text-[11px] text-fg-subtle">
                {index + 1}
              </span>
              {index === 0 && firstMark !== undefined && <Badge tone="info">{firstMark}</Badge>}
              <CommitInput
                aria-label={label(index + 1)}
                value={item}
                maxLength={LINE_LENGTH}
                onCommit={(text) =>
                  onChange((list) => list.map((old, at) => (at === index ? text : old)))
                }
                className="min-w-0 flex-1"
              />
              <IconButton
                icon={Trash2}
                tone="danger"
                label={removeLabel(index + 1)}
                onClick={() => onChange((list) => list.filter((_, at) => at !== index))}
              />
            </li>
          ))}
        </ol>
      ) : (
        empty
      )}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
        className="mt-3 flex items-center gap-2"
      >
        <Input
          aria-label={newLabel}
          value={draft}
          maxLength={LINE_LENGTH}
          placeholder={placeholder}
          disabled={full}
          onChange={(event) => setDraft(event.target.value)}
          className="min-w-0 flex-1"
        />
        <Button type="submit" icon={Plus} disabled={full || draft.trim() === ""}>
          {t("list.add")}
        </Button>
      </form>
      {full && (
        <p className="pt-1.5 text-[11.5px] text-fg-subtle">{t("list.full", { n: limit })}</p>
      )}
    </>
  );
}
