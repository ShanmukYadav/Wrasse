# Wrasse

A free, open-source disk cleaner for macOS that shows you what fills your Mac and helps you clear
it safely. It works offline: no account, no telemetry.

![The Wrasse home map](docs/images/home.png)

> **Beta.** Wrasse 0.1.0-beta is macOS only (Apple Silicon and Intel) and isn't signed by Apple
> yet, so macOS asks you to confirm the first launch (see Install). Windows comes later.

## What it does

- **A map of your disk.** Every folder on the Mac, grouped into Your Files, AI Models, Developer,
  Apps, System and Junk, with what each is and whether it's safe to clean. Open any block to go
  deeper, or switch to a list.
- **Clean up safely.** What you remove goes to a **tray** first and can be put back for 7 days
  (1–30, your choice). Wrasse re-checks every item right before it acts, and anything that can't
  be undone says so before you confirm.
- **Free Up Space.** Ask for "30 GB" and Wrasse suggests what to remove, best first: caches that
  rebuild themselves, apps you haven't opened in months, large old files, duplicates.
- **Apps, AI models, developer junk.** Uninstall apps with their leftovers; find local AI models
  from Ollama, LM Studio, Hugging Face and others, including duplicate copies; clear build
  folders, package caches and simulators.
- **Large & Old, duplicates, Startup Items, export**, a menu bar item, and a space report that
  explains why free space hasn't come back yet (local snapshots, purgeable space, the Trash).

![Free Up Space: what to remove for 30 GB](docs/images/free-up-space.png)

## Install

1. Download `Wrasse_0.1.0_universal.dmg` from the
   [latest release](https://github.com/ShanmukYadav/Wrasse/releases), open it and drag Wrasse to
   Applications.
2. Open Wrasse. Because this beta isn't signed by Apple, macOS says it can't verify it. Click
   **Done** (not Move to Trash).
3. Open **System Settings › Privacy & Security**, scroll to Security, and click **Open Anyway**
   next to "Wrasse was blocked". Confirm with your password or Touch ID. You only do this once.
4. **Full Disk Access** (recommended): Wrasse's first screen has a button that opens System
   Settings › Privacy & Security › Full Disk Access; turn Wrasse on there. Without it, Wrasse
   still works, but some folders (Mail, Safari, other apps' data) show as unreadable.

## What Wrasse never touches

macOS itself and other protected system places, your Keychains, Photos and Music libraries,
anything on your Never touch list, and iCloud files that aren't downloaded. Documents, Desktop and
iCloud Drive are never suggested: you pick from them by hand, and they always go to the tray. The
full list, and every program Wrasse may run, is in [SECURITY_AUDIT.md](SECURITY_AUDIT.md).

## Privacy

No telemetry, no analytics, no account. Wrasse's only network use: the update check, **off until
you turn it on** in Settings, and local calls to Ollama on your own Mac (`127.0.0.1`) when you use
AI models.

## Uninstall Wrasse completely

1. In Wrasse: **Settings › Remove all Wrasse data** (the tray, saved scans, logs, settings). It
   shows what will go first.
2. Quit Wrasse (menu bar item › Quit Wrasse) and move it from Applications to the Trash.
3. If you turned on **Open at login**, turn it off first in Settings, or remove Wrasse in System
   Settings › General › Login Items.
4. Nothing else remains. To check: `~/Library/Application Support/com.shanmukyadav.wrasse` should
   be gone (delete it if it's there).

## Reporting problems

- Something wrong or missing: [open an issue](https://github.com/ShanmukYadav/Wrasse/issues/new/choose).
- **Wrasse removed something it shouldn't**: look in the tray first (bottom right; Restore puts
  it back), then use that issue template.
- Security vulnerabilities: privately, see [SECURITY.md](SECURITY.md).

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). The
project is self-contained: Rust and Node live in `.toolchain/`, and everything runs through
`./dev.sh`.

```sh
# One-time setup on macOS (installs into ./.toolchain only)
mkdir -p .toolchain/node
curl -fsSL https://nodejs.org/dist/v24.21.0/node-v24.21.0-darwin-arm64.tar.gz \
  | tar -xz -C .toolchain/node --strip-components=1
curl -fsSL https://sh.rustup.rs | ./dev.sh sh -s -- -y --no-modify-path --profile minimal

# Run the app
cd app
../dev.sh npm install
../dev.sh npm run tauri dev
```

## License

MIT ([LICENSE](LICENSE)). The name and fish mark: see [TRADEMARK.md](TRADEMARK.md). Changes:
[CHANGELOG.md](CHANGELOG.md).
