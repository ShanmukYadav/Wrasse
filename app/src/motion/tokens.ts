// The only durations, easings and springs the UI may use (PLAN.md §7.2).
export const ease = {
  out: [0.22, 1, 0.36, 1], // entering
  inOut: [0.65, 0, 0.35, 1], // exits, crossfades
} as const;
export const duration = { fast: 0.18, base: 0.26, slow: 0.4 } as const;
export const spring = {
  soft: { type: "spring", stiffness: 260, damping: 30, mass: 0.9 },
  snappy: { type: "spring", stiffness: 420, damping: 34 },
  gentle: { type: "spring", stiffness: 140, damping: 22 },
} as const;
export const stagger = 0.035;

// Reef Map timings (seconds).
export const mapMotion = {
  sonar: 2.5, // radial scan sweep
  sonarFeather: 90, // px: width of the band where blocks fade and scale in
  pulse: 1.5, // "where is" highlight
  candidateStagger: 0.045, // "free N GB" candidates light up in ranked order
  flight: 0.7, // block flying into the quarantine tray
  wheelThreshold: 60, // accumulated wheel delta that triggers one zoom step
  caustics: 48, // one slow drift cycle of the backdrop light
  bubbleRise: 1.8, // seconds for a scan bubble to rise and fade
} as const;
