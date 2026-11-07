# ADR 0006: Procedural, layered materials baked over a seamed render mesh

**Status:** accepted

## Context

M4 needs a believable, editable skin and eyes. The CC0 data set has UVs but
no skin textures; shipping or fetching photographic textures raises licence
and size questions, and fixed textures don't adapt to custom looks.

## Decision

- **Render mesh.** Keep the welded base mesh as the identity (morphs, weights,
  projects). Add a `RenderMesh` with vertices split at UV seams and a map back
  to base vertices; everything is evaluated on the base mesh and gathered.
  Triangle order is shared, so triangle *t* is the same face in both.
- **Bake, don't shade.** A CPU layer stack evaluated per texel produces plain
  PBR textures (albedo, tangent-space normal, ORM). The same textures drive
  the viewport and the export, so what you see is what engines get.
- **Surface-space evaluation.** The UV layout is rasterized once into a
  texel → (triangle, barycentric) map; noise is evaluated at the **unmorphed**
  3D surface point. Patterns are continuous across seams, don't stretch with
  UVs, and don't change with body shape (so morphing never re-bakes).
- **Regions from the rig.** Lips, eyelids, cheeks and brows are sums of skin
  weights of face bones; no painted masks to author or license.
- **Tangents.** Our own per-vertex tangents along +u with handedness for the
  glTF convention (+Y up the image), exported as `TANGENT` and used in the
  viewport, matching how the normal map is baked.
- **Eyes.** Generated UV spheres on the `eye.L`/`eye.R` bones (size from the
  inter-eye distance), polar UV layout so the iris needs no seam, procedural
  iris texture; rigidly skinned to the eye bone in exports.

## Consequences

- Looks are parameters, saved in projects (small) and undoable.
- Bake cost (≈0.2 s at 1024² on 4 cores) means re-baking on slider release,
  not per frame; GPU compositing can replace the CPU path later behind the same
  layer model.
- Image layers (decals, tattoos, user textures) fit the same stack later as a
  new `Source` variant.
