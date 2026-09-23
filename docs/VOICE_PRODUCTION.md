# POWDERBURN dialogue voice production

The campaign dialogue is authored in the scenario RON files. Pre-battle lines
are `prebattle_dialogue`; combat barks are `battle_dialogue`. Every combat bark
is tied to a real simulation event and remains valid when its voice field is
empty, so missing audio never changes the deterministic battle.

## Clip contract

Generated clips belong in `assets/audio/dialogue/`. The runtime accepts either
the existing WAV assets or compressed OGG assets. OGG is preferred for a large
spoken campaign because it keeps the voice pack small; use short mono clips,
consistent loudness, and a little headroom so the score ducking remains clear.

When a clip is ready, set its filename in the line's `voice` field. The desktop
client plays the clip, ducks the score while it is active, and keeps the
speaker-labelled subtitle visible for accessibility. A line without a clip
still renders as a battlefield dialogue panel.

The existing `scripts/generate-voices.ps1` is an offline Windows speech
fallback for draft coverage. It is not presented as the final character voice
set; AI/TTS clips can replace those files without changing the dialogue schema
or combat code.

## Suggested review loop

1. Generate one clip per authored line with stable filenames.
2. Normalize and compress the clips before placing them under the dialogue
   asset directory.
3. Fill the corresponding `voice: Some("...")` field.
4. Run the content validator and the app tests to confirm every referenced
   clip exists and every combat trigger remains legal.
