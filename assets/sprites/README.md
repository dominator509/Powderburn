# Combat Sprite Art

`frontier_company_atlas_v2.png` is the production combat identity sheet used
by `pb-render`. It contains a 4x2 grid of eight distinct, period-grounded
company members. The renderer selects identity cells deterministically from
actor IDs. Stance, casualty, faction, routing, and selection state are applied
through geometry, alpha, and restrained runtime tinting.

Faction and selection colors are applied as a subtle runtime tint:

- Player: #4a90d9 (blue)
- Enemy: #d94a4a (red)
- Neutral: #d9c84a (yellow)
- Terrain: #4ad94a (green) with variation per elevation

## Provenance

The atlas was generated specifically for POWDERBURN using OpenAI ImageGen on
2026-07-27, then converted from a flat chroma-green field to a soft transparent
matte with the project image-generation helper. Full generation and licensing
details are recorded in `assets/PROVENANCE.toml`.

## Atlas Format

The sheet is a 4-column by 2-row PNG. UV selection is defined by
