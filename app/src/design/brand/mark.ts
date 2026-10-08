// The Wrasse mark: a minimal reef fish from three curves (body, tail, cleaner stripe) and an eye,
// on the same 24×24 grid as the icons. Reads at 16px; scales to the 512px+ app icon.
export const MARK = {
  body: "M2.5 12C6 6.2 13.6 5.6 18 11.3c.3.4.3 1 0 1.4C13.6 18.4 6 17.8 2.5 12z",
  tail: "M17.4 12 21.4 7.7c.4-.4 1.1-.1 1.1.5v7.6c0 .6-.7.9-1.1.5z",
  stripe: "M4.8 12.5c3.9 1.6 8.3 1.7 12.2 0",
  eye: { cx: 6.5, cy: 10.9, r: 0.95 },
} as const;
