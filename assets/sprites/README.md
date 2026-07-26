# Placeholder Art

All placeholder art in this directory is flat-color silhouettes generated programmatically by `pbtool atlas pack` and `pbtool atlas gen-placeholder`.

## Faction Colors

- Player: #4a90d9 (blue)
- Enemy: #d94a4a (red)
- Neutral: #d9c84a (yellow)
- Terrain: #4ad94a (green) with variation per elevation

## Provenance

All placeholder sprites are project-authored and have no external license restrictions. They will be replaced by commissioned art before the 1.0 release.

## Atlas Format

Atlas is a packed RGBA PNG with each sprite labeled by its asset key. The atlas index is generated alongside and stored as JSON.

## Tools

- `pbtool atlas pack` - pack individual sprites into an atlas
- `pbtool atlas gen-placeholder` - generate placeholder sprite sheets
