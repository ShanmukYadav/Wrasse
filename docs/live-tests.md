# Live tests (irreversible) — Phase 6 release checklist

These run once per release candidate, **by the owner, clicking in the app**, on disposable
targets set up for the test. They are unit-tested only until then. Nothing in this file is run by
an agent (AGENTS.md, "Live actions on the owner's files").

For every test:
- Use a release build. Turn **Dry run** off (the badge reads LIVE).
- Before and after, record the command output named in **Check**, and the matching audit-log line
  (`~/Library/Application Support/com.shanmukyadav.wrasse/logs/actions.log`, origin `ui`).
- A test passes only if every expected result holds. Anything else stops the release.

## 1. Homebrew cask uninstall

- **Target:** a small free cask nobody uses on the test Mac (check the size with
  `brew info --cask <token>` first). Install it with `brew install --cask <token>`.
- **Steps:** Scan. Apps → the app → Uninstall. The sheet says "Removed with Homebrew" and "Not
  undoable". Keep the bundle-id leftovers ticked. Confirm.
- **Expected:** `brew list --cask` no longer lists the token. The bundle is gone from
  `/Applications`. The ticked leftovers are in the tray and restore from there. The audit line
  reads `brew uninstall --cask <token>`. `brew` wasn't run with `--zap`.

## 2. Finder Move to Trash (app installed for all users)

- **Target:** a root-owned copy of the dummy app.
  `scripts/dummy-app.sh create`, then
  `sudo cp -R ~/Applications/WrasseDummy.app /Applications/WrasseRootDummy.app` and
  `sudo chown -R root:wheel /Applications/WrasseRootDummy.app`.
- **Steps:** Scan. Apps → WrasseRootDummy → Uninstall. The sheet says Finder will ask for the
  password and the app goes to the Trash. Confirm, then enter the password in Finder's prompt.
- **Expected:** the bundle is in the Trash, and Finder's Put Back returns it to
  `/Applications`. Cancelling the password prompt leaves it in place, and Wrasse reports a failure,
  not success.
- **Clean up:** empty it from the Trash by hand, or `sudo rm -rf` the put-back copy.

## 3. `docker system prune`

- **Target:** Docker running. `docker run --name wrasse-test hello-world` (leaves a stopped
  container). `docker volume create wrasse-keep` (a volume that must survive).
- **Check:** `docker ps -a`, `docker images -f dangling=true`, `docker volume ls`.
- **Steps:** Scan. Dev → Docker → its cleanup action. Confirm.
- **Expected:** the stopped `wrasse-test` container and dangling images are gone.
  `wrasse-keep` and running containers are untouched (no `--volumes`). With Docker not running,
  the action is unavailable rather than failing.

## 4. `brew cleanup -s`

- **Target:** `brew fetch --force <small formula>` to put a download in the Homebrew cache.
- **Check:** `brew cleanup -n -s` (the dry list), `du -sh "$(brew --cache)"`, `brew list`.
- **Steps:** Scan. Dev → Homebrew → cleanup. Confirm.
- **Expected:** the listed cache files are gone, and the cache size drops by about what the dry
  list said. `brew list` is identical before and after: Homebrew and installed formulae stay.

## 5. Snapshot thinning

- **Target:** `tmutil localsnapshot` (one fresh local snapshot), on a Mac with Time Machine set up.
- **Check:** `tmutil listlocalsnapshots /`, and `df -k /System/Volumes/Data`.
- **Steps:** Scan. The dashed "held by snapshots" tile shows space. Free now. Confirm.
- **Expected:** fewer local snapshots listed, and free space rises by about the amount shown. A
  password prompt appears only if thinning without one failed. The audit line reads
  `tmutil thinlocalsnapshots / <bytes> 4`.

## 6. Deleting unavailable simulators

- **Target:** with Xcode, create a simulator on a runtime, then delete that runtime
  (`xcrun simctl runtime delete <id>`) so the simulator becomes unavailable.
- **Check:** `xcrun simctl list devices unavailable`, `xcrun simctl list devices available`.
- **Steps:** Scan. Dev → Simulators → delete unavailable. Confirm.
- **Expected:** the unavailable list is empty, and the available list is unchanged.

## 7. `ollama rm`

- **Target:** Ollama running. `ollama pull smollm:135m` (a small model; check its size with
  `ollama list`).
- **Check:** `ollama list`, and `du -sh ~/.ollama/models`.
- **Steps:** Scan. AI Models → smollm:135m → Remove. The sheet says "Not undoable". Confirm.
- **Expected:** `ollama list` no longer shows it, and the models folder shrinks by about its
  size. Other models are untouched.

## 8. Ollama and LM Studio discovery

- **Target:** the small model from test 7 pulled again in Ollama, and a small GGUF (around
  100 MB) downloaded in LM Studio.
- **Steps:** Scan. Open AI Models. Load each model once (`ollama run`, LM Studio's chat), then
  check the loaded-model view. Unload from Wrasse.
- **Expected:** both appear with the right tool, size and path. If they share weights, the copy is
  shown as a duplicate, and removal confirms it byte for byte first. The loaded view shows each
  while loaded, and unloading frees its memory without deleting anything (`ollama list` still
  shows it).
