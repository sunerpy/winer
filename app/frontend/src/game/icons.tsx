// Game imagery from the connected client, with a quiet fallback when there is no client.
import { useState } from "react";

import { assetUrl, championIconUrl, profileIconUrl } from "../lib/assets";
import { cx } from "../lib/cx";
import { useT } from "../lib/i18n";
import { useAugmentDetails, useCatalog } from "../lib/store";

/** The first character as a reader sees it: one grapheme, so an emoji or a combined glyph stays whole. */
function initials(name: string): string {
  const [first] = new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(name.trim());
  return (first?.segment ?? "").toUpperCase();
}

/** An image that falls back to initials (or an empty tile) when it cannot load. `ground` is what
 *  shows through a transparent image, and the initials' colour on it. */
function GameImage({
  src,
  alt,
  size,
  fallback,
  ground = "bg-inset2 text-fg-subtle",
  className,
}: {
  src: string | undefined;
  alt: string;
  size: number;
  fallback?: string;
  ground?: string;
  className?: string;
}) {
  const [failed, setFailed] = useState<string | undefined>();
  const broken = !src || failed === src;
  return (
    <span
      className={cx(
        "relative inline-grid shrink-0 place-items-center overflow-hidden",
        ground,
        className,
      )}
      style={{ width: size, height: size }}
      title={alt || undefined}
    >
      {broken ? (
        <span aria-hidden className="font-medium" style={{ fontSize: Math.max(9, size * 0.38) }}>
          {fallback}
        </span>
      ) : (
        <img
          src={src}
          alt={alt}
          width={size}
          height={size}
          draggable={false}
          loading="lazy"
          onError={() => setFailed(src)}
          className="size-full object-cover"
        />
      )}
    </span>
  );
}

/** The game's dark ground, under icons drawn as light line art for it (tokens.css). */
const GLYPH_GROUND = "bg-glyph-plate text-glyph-ink";

export function ChampionIcon({
  id,
  size = 32,
  intent = false,
  className,
}: {
  id: number;
  size?: number;
  /** A declared intent, not a pick: drawn faded with a dashed ring. */
  intent?: boolean;
  className?: string;
}) {
  const champion = useCatalog()?.champions.get(id);
  const name = champion?.name ?? "";
  return (
    <GameImage
      src={championIconUrl(id)}
      alt={name}
      size={size}
      fallback={initials(champion?.shortName ?? name)}
      className={cx(
        "rounded-6 shadow-[inset_0_0_0_1px_var(--border)]",
        intent && "opacity-60 outline-1 outline-offset-1 outline-fg-subtle outline-dashed",
        className,
      )}
    />
  );
}

export function ProfileIcon({
  id,
  size = 40,
  className,
}: {
  id: number;
  size?: number;
  className?: string;
}) {
  return (
    <GameImage
      src={profileIconUrl(id)}
      alt=""
      size={size}
      className={cx("rounded-full shadow-[inset_0_0_0_1px_var(--border)]", className)}
    />
  );
}

type AssetKind = "items" | "spells" | "perks";

/** An item, summoner spell or rune by id; an empty slot for id 0. */
export function AssetIcon({
  kind,
  id,
  size = 22,
  className,
}: {
  kind: AssetKind;
  id: number;
  size?: number;
  className?: string;
}) {
  const asset = useCatalog()?.[kind].get(id);
  // An empty slot, or one the catalog cannot name yet: a plate with nothing on it would read as a
  // broken icon.
  if (id <= 0 || !asset?.icon) {
    return (
      <span
        aria-hidden
        className={cx("inline-block shrink-0 rounded-4 bg-inset2", className)}
        style={{ width: size, height: size }}
      />
    );
  }
  return (
    <GameImage
      src={assetUrl(asset?.icon)}
      alt={asset?.name ?? ""}
      size={size}
      // Runes are line art like augments; items and spells are square paintings with their own.
      ground={kind === "perks" ? GLYPH_GROUND : undefined}
      className={cx(kind === "perks" ? "rounded-full" : "rounded-4", className)}
    />
  );
}

const RARITY_RING = {
  silver: "shadow-[0_0_0_1.5px_var(--rarity-silver)]",
  gold: "shadow-[0_0_0_1.5px_var(--rarity-gold)]",
  prismatic: "shadow-[0_0_0_1.5px_var(--rarity-prismatic)]",
} as const;

/** An Arena or Hextech ARAM augment: the client's icon on the game's dark plate, in a ring of its
 *  rarity, with its name, rarity and (when ARAM.GG has it) what it does as the tooltip. */
export function AugmentIcon({
  id,
  size = 18,
  className,
}: {
  id: number;
  size?: number;
  className?: string;
}) {
  const t = useT();
  const augment = useCatalog()?.augments.get(id);
  const description = useAugmentDetails()?.get(id);
  const name = augment?.name || `#${id}`;
  const heading = augment?.rarity ? `${name} · ${t(`augment.${augment.rarity}`)}` : name;
  return (
    <span
      role="img"
      aria-label={name}
      title={description ? `${heading}\n${description}` : heading}
      className={cx(
        "inline-grid shrink-0 rounded-4",
        augment?.rarity && RARITY_RING[augment.rarity],
        className,
      )}
    >
      <GameImage
        src={assetUrl(augment?.icon)}
        alt=""
        size={size}
        fallback={initials(name)}
        ground={GLYPH_GROUND}
        className="rounded-4"
      />
    </span>
  );
}
