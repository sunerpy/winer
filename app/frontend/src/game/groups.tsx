// The six group colours (tokens.css, `--group-*`), taken in turn by a group's number: friends in
// one game or party, a premade party in a game. Always next to the number, never the only sign.
import { cx } from "../lib/cx";
import { Badge } from "../ui";

const DOTS = [
  "bg-group-1",
  "bg-group-2",
  "bg-group-3",
  "bg-group-4",
  "bg-group-5",
  "bg-group-6",
] as const;

const STRIPES = [
  "shadow-[inset_3px_0_0_0_var(--group-1)]",
  "shadow-[inset_3px_0_0_0_var(--group-2)]",
  "shadow-[inset_3px_0_0_0_var(--group-3)]",
  "shadow-[inset_3px_0_0_0_var(--group-4)]",
  "shadow-[inset_3px_0_0_0_var(--group-5)]",
  "shadow-[inset_3px_0_0_0_var(--group-6)]",
] as const;

/** Which of the six colours group `group` (1 up) wears. */
export function groupSlot(group: number): number {
  return (((Math.trunc(group) - 1) % DOTS.length) + DOTS.length) % DOTS.length;
}

/** The background utility of group `group`'s colour, for a dot. */
export function groupDot(group: number): string {
  return DOTS[groupSlot(group)] ?? DOTS[0];
}

/** An inset stripe on the left edge in group `group`'s colour, for a row. */
export function groupStripe(group: number): string {
  return STRIPES[groupSlot(group)] ?? STRIPES[0];
}

/** `■ 开黑 1`: a neutral badge led by the group's colour. */
export function GroupBadge({
  group,
  label,
  title,
}: {
  group: number;
  label: string;
  title?: string;
}) {
  return (
    <Badge title={title}>
      <span
        aria-hidden
        data-group={groupSlot(group) + 1}
        className={cx("size-2 shrink-0 rounded-[2px]", groupDot(group))}
      />
      {label}
    </Badge>
  );
}
