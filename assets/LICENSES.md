# Asset license ledger

Every asset used by OxyCharacter must be listed here (see the clean-room policy
in docs/PLAN.md §0). Fetched assets are **not** committed; they are downloaded
by `tools/fetch-assets.sh` into `assets/cache/` and converted into
`assets/generated/` (both git-ignored).

| Asset | Source | License | Notes | Date added |
|---|---|---|---|---|
| MakeHuman hm08 base mesh (`data/3dobjs/base.obj`, `body` group only) | https://github.com/makehumancommunity/makehuman | CC0-1.0 (released Sept 2020, see upstream `LICENSE.ASSETS.md`) | Fetched via sparse checkout of data dirs only; AGPL program code is never downloaded | 2026-11-07 |
| MakeHuman morph targets (`data/targets/**`) | same | CC0-1.0 | Expression, genital and body-type-dependent breast targets are not imported in M1 | 2026-11-07 |
| `docs/images/*.png` | Rendered by OxyCharacter from the CC0 assets above | CC0-1.0 | Editor screenshots | 2026-11-07 |
