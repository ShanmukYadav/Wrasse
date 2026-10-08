export const en = {
  "app.name": "Wrasse",
  "app.dev": "DEV",
  "app.devTooltip": "Development build: destructive actions default to dry-run",

  "nav.home": "Smart Scan",
  "nav.freeUpSpace": "Free Up Space",
  "nav.storage": "Storage",
  "nav.apps": "Apps",
  "nav.aiModels": "AI Models",
  "nav.developer": "Developer",
  "nav.largeOld": "Large & Old Files",
  "nav.startup": "Startup Items",
  "nav.diagnostics": "Diagnostics",
  "nav.quarantine": "Quarantine",
  "nav.settings": "Settings",
  "nav.label": "Sections",

  "screen.home.body": "Scan for junk, developer caches, AI models and large files in one pass.",
  "screen.freeUpSpace.body": "Pick how much space you need and get a ranked list of what to remove.",
  "screen.storage.body": "See exactly what is using your disk, and where it lives.",
  "screen.apps.body": "Find each app's true size, including its leftovers, and uninstall cleanly.",
  "screen.aiModels.body": "Find local models from Ollama, LM Studio and Hugging Face, including duplicates.",
  "screen.developer.body": "Clear old node_modules, build caches, package caches and stale environments.",
  "screen.largeOld.body": "Find big files, files you haven't opened in a year, and duplicates.",
  "screen.startup.body": "See what starts when you log in and turn items off.",
  "screen.diagnostics.body": "Find out why deleted space hasn't come back, and fix it where possible.",
  "screen.quarantine.body": "Restore anything Wrasse moved aside in the last 7 days.",
  "screen.settings.body": "Rules, quarantine, never-touch list, motion and updates.",
  "screen.comingSoon": "Coming in a later phase",
} as const;

export type MessageKey = keyof typeof en;
