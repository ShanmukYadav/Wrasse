import { useLayoutEffect, useRef, useState } from "react";

let measure: CanvasRenderingContext2D | null = null;

/** Truncates in the middle ("Llama 3.1 70B · LM…Studio") so both ends stay readable. */
export function MiddleEllipsis({ text, className }: { text: string; className?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [shown, setShown] = useState(text);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const fit = () => {
      const width = el.clientWidth;
      const style = getComputedStyle(el);
      measure ??= document.createElement("canvas").getContext("2d");
      if (!measure || width <= 0) return setShown(text);
      measure.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      if (measure.measureText(text).width <= width) return setShown(text);
      let lo = 0;
      let hi = text.length;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        const candidate = text.slice(0, Math.ceil(mid / 2)) + "…" + text.slice(text.length - Math.floor(mid / 2));
        if (measure.measureText(candidate).width <= width) lo = mid;
        else hi = mid - 1;
      }
      setShown(text.slice(0, Math.ceil(lo / 2)) + "…" + text.slice(text.length - Math.floor(lo / 2)));
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [text]);

  return (
    <span ref={ref} className={className} title={shown === text ? undefined : text}>
      {shown}
    </span>
  );
}
