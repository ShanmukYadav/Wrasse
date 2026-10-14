# Final test on a fresh macOS user account

Run once per release candidate, by you, after the draft release exists. It checks what a new
user meets: the download, Gatekeeper, onboarding, a scan, cleaning, the tray, and removing
Wrasse. Write down anything that surprises you; that's the result I need.

## Before

1. Create a standard user: System Settings › Users & Groups › Add User. Name it `wrassetest`.
2. Log in as `wrassetest` (log out of your own account first, or use fast user switching).

## In the test account

1. **Download:** open the draft release page in Safari (sign in to GitHub there if the release is
   still a draft) and download `Wrasse_0.1.0_universal.dmg`.
2. **Install:** open the `.dmg`, drag Wrasse to Applications, eject the `.dmg`.
3. **First launch:** open Wrasse from Applications. Expected: macOS says it can't verify Wrasse.
   Click **Done**. Open System Settings › Privacy & Security, click **Open Anyway** next to
   "Wrasse was blocked", confirm. Wrasse opens.
   - **Take a screenshot of the Privacy & Security section with the Open Anyway button**
     (⌘⇧4, then Space, click the window) and send it to me: it goes in the README.
4. **Onboarding:** read the first screen. Use its button to turn on Full Disk Access, then come
   back. Expected: the first scan starts by itself and the map appears.
5. **Trial files:** in Terminal, paste:
   ```sh
   mkdir -p ~/WrasseTrial/big ~/WrasseTrial/old && cd ~/WrasseTrial
   mkfile 600m big/trial-video.mov && mkfile 120m old/trial-archive.zip
   cp old/trial-archive.zip "old/trial-archive copy.zip" && touch -t 202301010000 old/*.zip
   find . -type f -print0 | sort -z | xargs -0 shasum -a 256 > ~/WrasseTrial-checksums.txt
   ```
   Then Scan again (⌘R).
6. **Clean:** Large & Old (⌘K "large"): find `trial-video.mov`, tick it, Move to the tray.
   Duplicates: the two `trial-archive` zips show as a pair; keep one, move the other.
   Expected: the tray (bottom right) shows 2 items, about 720 MB; the free space in the header
   doesn't change yet (the tray still holds them).
7. **Restore:** open the tray, Restore all. Then in Terminal:
   `cd ~/WrasseTrial && shasum -a 256 -c ~/WrasseTrial-checksums.txt`. Expected: every line OK.
8. **Free Up Space:** ⌘K "free 1 GB". Look at the plan; don't run it. Expected: it explains each
   suggestion and never offers Documents or Desktop items.
9. **Menu bar:** close the window (red button). Expected: the fish stays in the menu bar; its
   menu shows free space; Open Wrasse brings the window back.
10. **Export:** ⌘K "export scan summary as Markdown". Expected: a file in Downloads, shown in Finder.
11. **Update check:** Settings › Check for updates on, quit and reopen. Expected: no update offer
    (there's nothing newer), no error.
12. **Remove Wrasse:** Settings › Remove all Wrasse data (read what it lists, confirm). Quit from the
    menu bar. Move Wrasse from Applications to the Trash. In Terminal:
    `ls ~/Library/Application\ Support/com.shanmukyadav.wrasse` should say "No such file".
13. Clean up: `rm -rf ~/WrasseTrial ~/WrasseTrial-checksums.txt`.

## After

Log back into your own account and delete the `wrassetest` user (Users & Groups › the user ›
Delete User, "Delete the home folder").

## Report back

For each step: as expected, or what you saw instead (a screenshot helps). Plus the Open Anyway
screenshot from step 3.
