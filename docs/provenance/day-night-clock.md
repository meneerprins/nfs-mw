# Day-night clock

- **Spec:** docs/specs/time-of-day.md (clock and sun sections)
- **Sources read for the spec:** the retail PC executable `speed.exe` (Ghidra, base 0x400000, read and
  disassembled locally), the installed `GLOBAL/ATTRIBUTES.BIN`, and live CheatEngine checks in free roam. The
  day-night settings names were taken from this repository's own note of the behaviour (docs/mods), which lists
  the key names only; no code of any mod was read or ported.
- **Implemented:** the clock's curves in `crates/nfsmw-data/src/day_night.rs` and the world wiring in
  `crates/nfsmw/src/scenes/world/day.rs`, from the spec only. The default values (noon start, 150 s per game
  hour, sunrise 06:00, sunset 18:00, lights on 18:30, lights off 06:30, azimuth 3.28) are this project's choice,
  not install values.
- **Checked against the game by:** unit tests only (clock steps to night, night takes the low-sun presets, lights
  at the configured hours). Not yet compared with the original game at a known hour.
- **Known differences from the original:**
  - The writer of the clock counter (`0x009B0FE4`) is identified, but the state conditions that enable the
    original clock are not confirmed. The clock here runs from the start in every state (see the spec).
  - `RealTime` and `RandomStartupTime` are read but not applied.
  - The phase jumps at midnight (from the high limit to the low limit). The night is the low-sun limit, which
    selects the low-sun presets; the preset index that is the original night is not confirmed.
  - World lights (street lights) are not switched; only the player's headlights.
  - The sky colour is not driven by the clock.
- **Time scale (E2):** the clock is fed the simulation delta, so pause, loading and the garage hold it, the
  speedbreaker dilates it, and GameSpeed scales it (kept on purpose). Own wiring from the spec and the existing
  speedbreaker clock, no community mod code read or copied. Checked by synthetic tests in `day.rs` only. The
  mod's pause and time-scale facts (IsPaused, the sim time-scale field, the frame detour) were not
  cross-checked against the executable in this pass and are not relied on.

## Career event lighting and frontend transitions

Both inspected PC installs declare TOD 1 for the opening D-Day race, the two sprints and the final Razor race,
and 0.5 for Rog's circuit. The meeting between Rog and Razor inherits -1. In a stock career run the five green-light
phases were 0.909, 0.941, 0.941, 0.481 and 0.938. All five had a negative rain override (dry). Direct event launches
omitted the career phase setter. A normal safehouse round trip preserved phase 0.3856 on both sides of SetRoaming.
Cold safehouse entry seeds the observed initial orbit (approximately 0.45, moving toward dawn).
The car-lot jump and first-safehouse movie path request zero; the latter is confirmed statically.

The restored cinematic lighting uses the four data fields described in the [ENISTimeOfDay reference](../mods/nfsmw_enistimeofday.md).
Camera key entries dispatch the active sequence's ICECut tags; stock prologue scene-time observations match the
authored key boundaries. Clearing residual overcast and a held update rate on frontend return and skip prevents an
interrupted shot from fixing the weather for the next drive.
