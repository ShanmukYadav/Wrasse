import { useSpring } from "motion/react";
import { useEffect, useRef } from "react";
import { formatBytes } from "../lib/format";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";

// A byte count that counts up to its value, formatted live with tabular numerals.
// Text is written straight to the DOM each frame, so React doesn't re-render while it counts.
export function Bytes({
  value,
  className = "",
  unitClassName = "",
}: {
  value: number;
  className?: string;
  unitClassName?: string;
}) {
  const units = useApp((s) => s.info?.units ?? "decimal");
  const numberRef = useRef<HTMLSpanElement>(null);
  const unitRef = useRef<HTMLSpanElement>(null);
  const animated = useSpring(0, spring.gentle);

  useEffect(() => {
    const render = (v: number) => {
      const f = formatBytes(v, units);
      if (numberRef.current) numberRef.current.textContent = f.value;
      if (unitRef.current) unitRef.current.textContent = ` ${f.unit}`;
    };
    render(animated.get());
    const unsubscribe = animated.on("change", render);
    animated.set(value);
    return unsubscribe;
  }, [animated, units, value]);

  const final = formatBytes(value, units);
  return (
    <span className={`tabular ${className}`} aria-label={`${final.value} ${final.unit}`}>
      <span ref={numberRef} aria-hidden />
      <span ref={unitRef} className={unitClassName} aria-hidden />
    </span>
  );
}
