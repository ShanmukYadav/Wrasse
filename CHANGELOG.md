# Changelog

All notable changes to Wrasse. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-beta.1] - 2026-10-14

The first public beta, macOS only, not signed by Apple (see the README for the "Open Anyway" step).

### Added
- A map of the whole disk (D1 "Reef Blocks"): categories, drill-in, a list view, insight chips.
- Cleanup through one safety layer: protected places, a Never touch list, re-checks right before
  acting, and a tray where removed items wait (1–30 days) before they're deleted.
- Junk rules, apps and their leftovers with an uninstaller, Free Up Space, Large & Old,
  duplicates, AI models (Ollama, LM Studio, Hugging Face and more), developer caches and
  projects, space held by local snapshots.
- Startup Items, Settings, export (Markdown, CSV, JSON), a menu bar item, shortcuts.
- Opt-in update check (off by default), with signed updates.
- No telemetry. The only network use is the update check when you turn it on, and local calls to
  Ollama.
