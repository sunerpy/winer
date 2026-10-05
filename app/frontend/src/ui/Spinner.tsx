import { cx } from "../lib/cx";

export function Spinner({
  size = 14,
  className,
  label,
}: {
  size?: number;
  className?: string;
  label?: string;
}) {
  return (
    <span
      role={label ? "status" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cx(
        "inline-block shrink-0 rounded-full border-2 border-current border-r-transparent opacity-80",
        className,
      )}
      style={{ width: size, height: size, animation: "winer-spin 0.8s linear infinite" }}
    />
  );
}
