import { useId } from "react";
import { useScheme } from "../../lib/prefs";
import { brand, icon as iconTokens, tones } from "../tokens";
import { MARK } from "./mark";

type Variant =
  | "duotone" // in-app mark, same style as the icon family
  | "solid" // filled silhouette for tray / menu bar (template-style)
  | "appIcon"; // fish on the navy-to-teal rounded square

type Props = { variant?: Variant; size?: number; ink?: "dark" | "light"; label?: string; className?: string };

export function BrandMark({ variant = "duotone", size = 24, ink, label = "Wrasse", className }: Props) {
  const scheme = useScheme();
  const id = useId().replace(/:/g, "");
  if (variant === "appIcon") {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={label} className={className}>
        <defs>
          <linearGradient id={`g${id}`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor={brand.iconFrom} />
            <stop offset="1" stopColor={brand.iconTo} />
          </linearGradient>
          <linearGradient id={`h${id}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={brand.highlight} />
            <stop offset="0.55" stopColor="transparent" />
          </linearGradient>
        </defs>
        <rect x="0" y="0" width="24" height="24" rx="5.4" fill={`url(#g${id})`} />
        <rect x="0" y="0" width="24" height="24" rx="5.4" fill={`url(#h${id})`} />
        <g transform="translate(2.4 2.4) scale(0.8)">
          <path d={MARK.body} fill={brand.fish} />
          <path d={MARK.tail} fill={brand.fish} />
          <path d={MARK.stripe} fill="none" stroke={brand.stripe} strokeWidth={1.6} strokeLinecap="round" />
          <circle {...MARK.eye} fill={brand.iconFrom} />
        </g>
      </svg>
    );
  }
  if (variant === "solid") {
    const color = brand.ink[ink ?? scheme];
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={label} className={className}>
        <mask id={`m${id}`}>
          <rect width="24" height="24" fill="white" />
          <path d={MARK.stripe} fill="none" stroke="black" strokeWidth={1.5} strokeLinecap="round" />
          <circle {...MARK.eye} fill="black" />
        </mask>
        <g mask={`url(#m${id})`} fill={color}>
          <path d={MARK.body} />
          <path d={MARK.tail} />
        </g>
      </svg>
    );
  }
  const color = tones[scheme].accent;
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={label} className={className}>
      <g fill={color} fillOpacity={iconTokens.fillOpacity}>
        <path d={MARK.body} />
        <path d={MARK.tail} />
      </g>
      <g fill="none" stroke={color} strokeWidth={iconTokens.stroke} strokeLinecap="round" strokeLinejoin="round">
        <path d={MARK.body} />
        <path d={MARK.tail} />
        <path d={MARK.stripe} />
      </g>
      <circle {...MARK.eye} fill={color} />
    </svg>
  );
}
