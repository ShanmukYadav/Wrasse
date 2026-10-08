# Wrasse

Open-source, offline disk cleaner and storage analyzer for macOS and Windows.

Wrasse has no telemetry. Its only network traffic is an opt-in update check, plus local calls to the Ollama API on `127.0.0.1`.

> Status: early development (Phase 0, scaffold). See [PLAN.md](PLAN.md) for the full spec.

## Development

The project is self-contained: Rust and Node live in `.toolchain/` and the npm cache in `.npm-cache/`, so deleting this folder removes everything. Prerequisite on macOS: Xcode Command Line Tools (`xcode-select --install`).

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

`./dev.sh <command>` runs any command with the local toolchain; `source ./dev.sh` puts it on your current shell's PATH. `./clean.sh` removes the dev app's data from `~/Library`.

## License

MIT
