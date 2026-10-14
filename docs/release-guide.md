# Releasing Wrasse

How a beta goes out. The update key steps are once only; the rest is per release.

## Once: the update signing key

Updates are signed so installed copies only accept updates made by you. The key is yours alone:
it never goes in the repository or the project folder.

1. In Terminal (the password you choose protects the key; keep it in your password manager):
   ```sh
   cd ~/Documents/Wrasse/app && ../dev.sh npx tauri signer generate -w ~/.wrasse-updater.key
   ```
   This writes `~/.wrasse-updater.key` (private) and `~/.wrasse-updater.key.pub` (public).
2. **Store the private key safely**, in two places:
   - your password manager (or the macOS Keychain as a secure note): the file's contents and its
     password;
   - an encrypted USB drive kept offline.
   Then delete `~/.wrasse-updater.key` from the Mac (`rm ~/.wrasse-updater.key`).
3. **GitHub secrets** (repository › Settings › Secrets and variables › Actions):
   - Secret `TAURI_SIGNING_PRIVATE_KEY`: the private key file's contents.
   - Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password.
   - Variable (not secret) `WRASSE_UPDATER_PUBKEY`: the contents of `~/.wrasse-updater.key.pub`.
     The app is built with it and checks every update against it.

If the private key leaks, anyone could sign an update: make a new key, put the new public key in
the variable, and ship a release people install by hand. If it's lost, installed copies can't
update; ship a new key the same way.

## Each release

1. Memory regression check against the previous release, on an untouched Mac: mount the previous
   release's `.dmg`, copy `Wrasse.app/Contents/MacOS/wrasse-app` somewhere, then run
   `scripts/check-memory-regression.sh <that copy>`. It must pass (within 5 MB). Also run
   `scripts/check-idle-memory.sh --launches 5` (idle 110 MB, peak 250 MB).
2. Update `CHANGELOG.md` (move "unreleased" to the version and date) and commit.
3. Tag and push: `git tag v0.1.0-beta.1 && git push origin v0.1.0-beta.1`.
4. The **Release** workflow runs the checks, builds the universal `.dmg` and the signed updater
   archive, and creates a **draft** release with them and `latest.json`.
5. Run the live tests ([live-tests.md](live-tests.md)) and the fresh-account test
   ([fresh-account-test.md](fresh-account-test.md)) on the draft's `.dmg`.
6. Edit the draft: check the notes, tick **Set as a pre-release** for betas, and **Publish**.
7. The **Update channel** workflow copies the release's `latest.json` to the `updates` release
   (`beta.json`, and `stable.json` for stable releases): installed copies with the update check on
   see it at their next launch.
