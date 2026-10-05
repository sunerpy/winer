// winer's mark, as `scripts/brand/icons.py` draws it: a gold hexagon with a W cut out of it. Up to
// 24px it has no plate and a navy outline, so it reads on a light surface as on a dark one; larger,
// it sits on the navy plate like the app icon. Its colours are the icon's own in every theme.
const SMALL = {
  hexagon: "M256.0,9.6 L469.4,132.8 L469.4,379.2 L256.0,502.4 L42.6,379.2 L42.6,132.8 Z",
  w: "M117.0,207.8 L190.5,413.4 L256.0,315.1 L321.5,413.4 L395.0,207.8 L329.0,184.2 L302.5,258.6 L256.0,188.9 L209.5,258.6 L183.0,184.2 Z",
};
const LARGE = {
  hexagon: "M256.0,51.0 L433.5,153.5 L433.5,358.5 L256.0,461.0 L78.5,358.5 L78.5,153.5 Z",
  w: "M134.5,213.3 L198.2,401.3 L256.0,308.7 L313.8,401.3 L377.5,213.3 L322.5,194.7 L298.2,266.7 L256.0,199.3 L213.8,266.7 L189.5,194.7 Z",
};

export function Logo({ size = 22, className }: { size?: number; className?: string }) {
  const small = size <= 24;
  const shape = small ? SMALL : LARGE;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 512 512"
      aria-hidden
      focusable="false"
      className={className}
    >
      {!small && <rect width="512" height="512" rx="112" fill="#0f1a26" />}
      <path
        d={shape.hexagon}
        fill="#e3bd66"
        stroke={small ? "#0f1a26" : undefined}
        strokeWidth={small ? 34 : undefined}
        strokeLinejoin="round"
      />
      <path d={shape.w} fill="#0f1a26" />
    </svg>
  );
}
