// Game art from the connected client (skins, challenge tokens), with a quiet plate where there is
// none: no client, or a picture the client does not have.
import { ImageOff } from "lucide-react";
import { useState } from "react";

import { assetUrl } from "../../lib/assets";
import { cx } from "../../lib/cx";

export function Art({
  path,
  alt,
  className,
  ground = "bg-inset2 text-fg-subtle",
}: {
  path: string | null | undefined;
  alt: string;
  className?: string;
  /** What shows through a transparent picture; challenge tokens are drawn for the game's dark one. */
  ground?: string;
}) {
  const [failed, setFailed] = useState<string | null>(null);
  const src = assetUrl(path);
  return (
    <span
      className={cx("grid shrink-0 place-items-center overflow-hidden", ground, className)}
      title={alt || undefined}
    >
      {src && failed !== src ? (
        <img
          src={src}
          alt={alt}
          loading="lazy"
          draggable={false}
          onError={() => setFailed(src)}
          className="size-full object-cover"
        />
      ) : (
        <ImageOff size={16} strokeWidth={1.75} aria-hidden />
      )}
    </span>
  );
}
