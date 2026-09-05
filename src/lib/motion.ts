// Shared framer-motion timing constants. Import these instead of writing inline objects.
export const MOTION = {
  // 160ms ease-out — standard page/section entry and tab content switch
  short: 0.16,

  // Spring for sliding indicators (tab underline, selection highlight)
  spring: { type: "spring", stiffness: 500, damping: 40 } as const,
};

// Fade + subtle Y lift — page entry and AnimatePresence tab content
// Use spread: <motion.div {...fadeY} />
export const fadeY = {
  initial:    { opacity: 0, y: 8 },
  animate:    { opacity: 1, y: 0 },
  exit:       { opacity: 0, y: -4 },
  transition: { duration: MOTION.short },
} as const;

// Same without exit — for elements that only enter (no AnimatePresence wrap)
export const fadeIn = {
  initial:    { opacity: 0, y: 8 },
  animate:    { opacity: 1, y: 0 },
  transition: { duration: MOTION.short },
} as const;

// Streaming cursor / AI-thinking pulse — distinct from skeleton, slightly slower
export const streamPulse = {
  animate: { opacity: [1, 0.35, 1] },
  transition: { duration: 0.9, ease: "easeInOut" as const, repeat: Infinity },
};
