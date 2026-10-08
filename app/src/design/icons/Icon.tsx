import { createElement, type CSSProperties } from "react";
import { icon as iconTokens, tones, type IconSize, type Tone } from "../tokens";
import { useScheme } from "../../lib/prefs";
import { ICONS, isFillable, type IconName } from "./nodes";

type Props = {
  name: IconName;
  /** Semantic color; omit to inherit the current text color. */
  tone?: Tone;
  size?: IconSize;
  className?: string;
  style?: CSSProperties;
  label?: string;
};

// The one icon component: duotone (outline + 20% fill in the same hue), rounded caps and joins.
export function Icon({ name, tone, size = 20, className, style, label }: Props) {
  const scheme = useScheme();
  const color = tone ? tones[scheme][tone] : "currentColor";
  const node = ICONS[name];
  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${iconTokens.grid} ${iconTokens.grid}`}
      className={className}
      style={{ flexShrink: 0, ...style }}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <g fill={color} fillOpacity={iconTokens.fillOpacity} stroke="none">
        {node.filter(isFillable).map(([tag, attrs], i) => createElement(tag, { key: i, ...omitStrokeAttrs(attrs) }))}
      </g>
      <g fill="none" stroke={color} strokeWidth={iconTokens.stroke} strokeLinecap="round" strokeLinejoin="round">
        {node.map(([tag, attrs], i) => createElement(tag, { key: i, ...camel(attrs) }))}
      </g>
    </svg>
  );
}

function camel(attrs: Record<string, string | number>) {
  const out: Record<string, string | number> = {};
  for (const [k, v] of Object.entries(attrs)) out[k.replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = v;
  return out;
}

function omitStrokeAttrs(attrs: Record<string, string | number>) {
  const { ["stroke-dasharray"]: _dash, ...rest } = attrs;
  void _dash;
  return camel(rest);
}
