import { useEffect, useState } from "react";
import type { Scheme } from "../design/tokens";
import { useApp } from "../store/app";

export function useScheme(): Scheme {
  const theme = useApp((s) => s.theme);
  const system = useApp((s) => s.systemScheme);
  return theme === "system" ? system : theme;
}

const reduceQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

/** Reduced motion from the in-app setting, falling back to the system preference. */
export function useReducedMotion(): boolean {
  const pref = useApp((s) => s.motion);
  const [system, setSystem] = useState(reduceQuery.matches);
  useEffect(() => {
    const handler = () => setSystem(reduceQuery.matches);
    reduceQuery.addEventListener("change", handler);
    return () => reduceQuery.removeEventListener("change", handler);
  }, []);
  return pref === "system" ? system : pref === "reduce";
}
