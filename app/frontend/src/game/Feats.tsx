// A line's feats (双杀 … 五杀, 超神, 一血, the game's leads, 逃兵): as chips with their names in a
// history row, as 18px marks on the scoreboard. What earned each is its tooltip.
import type { Feat } from "@winer/shared";
import type { LucideIcon } from "lucide-react";

import { cx } from "../lib/cx";
import { FEATS, type FeatLook, type FeatTone } from "../lib/feats";
import { useT } from "../lib/i18n";

const TONE: Record<FeatTone, string> = {
  double: "bg-feat-double-soft text-feat-double",
  triple: "bg-feat-triple-soft text-feat-triple",
  quadra: "bg-feat-quadra-soft text-feat-quadra",
  // The two rarest carry a ring and a sheen of their own colour.
  penta:
    "bg-linear-to-r from-feat-penta-strong to-feat-penta-soft text-feat-penta shadow-[inset_0_0_0_1px_var(--feat-penta)]",
  legend:
    "bg-linear-to-r from-feat-legend-strong to-feat-legend-soft text-feat-legend shadow-[inset_0_0_0_1px_var(--feat-legend)]",
  blood: "bg-feat-blood-soft text-feat-blood",
  best: "bg-best-soft text-best",
  away: "bg-danger-soft text-danger",
};

function useLabel() {
  const t = useT();
  return (look: FeatLook) => t("feat.label", { name: t(look.name), why: t(look.why) });
}

function FeatChip({ feat }: { feat: Feat }) {
  const t = useT();
  const label = useLabel();
  const look = FEATS[feat];
  const Icon = typeof look.icon === "number" ? null : look.icon;
  return (
    <span
      title={label(look)}
      className={cx(
        "inline-flex h-[18px] shrink-0 items-center gap-1 rounded-4 px-1.5 text-[10.5px] leading-none font-semibold whitespace-nowrap",
        TONE[look.tone],
      )}
    >
      {Icon && <Icon size={11} strokeWidth={2.2} aria-hidden />}
      {t(look.name)}
    </span>
  );
}

/** The first `limit` feats as chips and a count of the rest, which name themselves on hover. */
export function FeatChips({ feats, limit = 3 }: { feats: readonly Feat[]; limit?: number }) {
  const t = useT();
  if (feats.length === 0) return null;
  const rest = feats.slice(limit);
  return (
    <span aria-label={t("feat.list")} className="flex min-w-0 items-center gap-1 overflow-hidden">
      {feats.slice(0, limit).map((feat) => (
        <FeatChip key={feat} feat={feat} />
      ))}
      {rest.length > 0 && (
        <span
          title={rest.map((feat) => t(FEATS[feat].name)).join(" · ")}
          className="mono inline-flex h-[18px] shrink-0 items-center rounded-4 bg-inset px-1.5 text-[10.5px] leading-none text-fg-muted hairline"
        >
          +{rest.length}
        </span>
      )}
    </span>
  );
}

function MarkGlyph({ icon: Icon }: { icon: LucideIcon }) {
  return <Icon size={12} strokeWidth={2.2} aria-hidden />;
}

/** Every feat as an 18px mark: its glyph, or a multikill's count. */
export function FeatMarks({ feats }: { feats: readonly Feat[] }) {
  const t = useT();
  const label = useLabel();
  if (feats.length === 0) return null;
  return (
    <span aria-label={t("feat.list")} className="flex shrink-0 items-center gap-0.5">
      {feats.map((feat) => {
        const look = FEATS[feat];
        const { icon } = look;
        return (
          // `relative` keeps the hidden label inside the mark; loose, it would widen the document.
          <span
            key={feat}
            title={label(look)}
            className={cx(
              "relative inline-grid size-[18px] shrink-0 place-items-center rounded-4",
              TONE[look.tone],
            )}
          >
            {typeof icon === "number" ? (
              <span aria-hidden className="mono text-[10.5px] leading-none font-bold">
                {icon}
              </span>
            ) : (
              <MarkGlyph icon={icon} />
            )}
            <span className="sr-only">{label(look)}</span>
          </span>
        );
      })}
    </span>
  );
}
