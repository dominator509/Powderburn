# Changelog

All notable player-visible changes are recorded here. POWDERBURN follows semantic versioning over
its rules, content, interface, and save-format contracts.

## [Unreleased]

### Save compatibility

- No unreleased changes.

## [1.0.1] - 2026-07-27

### Rules

- Completed the deterministic tactical rules surface, including called shots, wounds, morale,
  environment effects, progression, and data-authored weapon and faction rules.

### Content

- Completed the four-act campaign, 24 missions, 12 camp interludes, companion arcs, historical
  citations, and representation validation.
- Added production art atlases, companion portraits, title art, frontier music, and gameplay cues.

### Interface

- Completed the keyboard-and-mouse app flow, tactical HUD, save/load, settings, accessibility
  palettes, text scaling, non-color overlay cues, headless capture, CLI, and content tools.

### Fixes

- Hardened deterministic replay, append-only Ledger verification, released-artifact smoke tests,
  reproducible packaging, detached signatures, immutable publication, and rollback recovery.

### Save compatibility

- Saves load only when format, ruleset, and content hashes match exactly. Mismatches are refused
  with a diagnostic; there is no silent migration.
