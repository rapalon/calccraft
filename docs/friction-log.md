# Friction log

What made work on CalcCraft slower than it had to be, newest first, so the next run (or a tool
change) can avoid it.

## 2026-11-24: finding the crash behind Google Play's rejection of 1.4.5 (task 3ec17022)

- **`monkey` doesn't count a process that ends as a crash.** CalcCraft ending itself
  (`exit(0)` after "EventLoop can't be recreated") died some 20 times in one run while monkey
  printed `// Monkey finished`. Only `adb logcat` (`has died: fg TOP`, `exited cleanly (0)`) and
  `adb shell dumpsys activity exit-info <package>` (`EXIT_SELF`) showed it. Read both after
  every monkey run, not just monkey's own `CRASH`/`ANR` lines.
- **A frozen app freezes the tools.** `adb shell input tap` waits for the hung app to take the
  touch, so a command that taps and then reads the log never returns; take the log in a
  separate command.
- **System changes reach every installed variant.** Changing the colour style (to test an
  overlay change) left every running CalcCraft build with a pending relaunch; monkey later
  opened the old `.pro.test` from the launcher, which froze, and stalled the run. Force-stop
  the other variants before such a test, and restore the setting at once.
- **`settings delete` doesn't apply.** Deleting `secure font_weight_adjustment` removes the row
  but the system keeps the last weight, so the next `put 300` changed nothing and looked like
  "Bold text doesn't restart the activity". Put the neutral value (`0`) first, then delete.
  `global always_finish_activities` (Don't keep activities) doesn't take effect through
  `settings put` at all.
- **The installed test builds are signed with another key**, so `adb install -r` of a local
  build failed (`INSTALL_FAILED_UPDATE_INCOMPATIBLE`) and the fix went in as a new application
  ID, `com.rapalon.calccraft.lifecycle`, to keep the other builds' data.
- **Runner permissions.** On pc_010, `adb` works only as bare `adb …`, one command at a time
  (its full path, `sh <script>`, compound commands with variables, and
  `pwsh -File tests/device/run.ps1` ask for approval), and the emulator couldn't be listed. So
  the device scenarios ran by hand with adb, and no other Android version or screen size was
  tried.
