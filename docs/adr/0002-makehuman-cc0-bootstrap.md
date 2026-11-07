# ADR 0002: Bootstrap the base human from MakeHuman's CC0 data

**Status:** accepted

We need a production-quality base mesh and morph library on day one. The
MakeHuman base mesh and targets were released CC0 in 2020. We use **only the
data files** (sparse checkout of `data/3dobjs` and `data/targets`); MakeHuman's
AGPL program code is never downloaded, read or used. The slider taxonomy,
target pairing heuristics, macro-grid model and all code are our own.

Data is fetched by `tools/fetch-assets.sh` and converted to our `.oxyb` bundle
rather than committed, keeping the repository small. M1 imports the `body`
group only; eyes, teeth and eyelashes come later from separate assets.

Interpretation of the data (from inspecting the files, not upstream code):
targets are sparse `index dx dy dz` deltas in decimetres; ancestry/gender/age
shapes live in `macrodetails/<ancestry>-<gender>-<age>`, muscle/weight
variations in `macrodetails/universal-*`, and height in `macrodetails/height/*`.
