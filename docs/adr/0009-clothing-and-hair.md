# ADR 0009: Clothing and hair fitting

**Status:** accepted

## Context

Garments and hair have to follow every body the sliders can produce, and then
the pose, without a cloth simulation. The starter content has to be ours (or
CC0); nothing commercial is available to us, and the CC0 body set has no
clothing meshes.

## Decision

- **Bind once, fit on every shape.** Each garment vertex stores the closest
  body triangle, the barycentric coordinates of that point, and the offset in
  the triangle's tangent frame (edge tangent, barycentric normal). Fitting
  rebuilds the vertex from the morphed triangle. On the body it was bound to,
  with tightness 0.5 and smoothing off, this reproduces the authored mesh.
- **Tightness** is centred at 0.5 (as authored). Higher pulls the offset
  toward the skin; lower lets it stand off. A few Laplacian steps, scaled by
  `(1 − tightness)`, relax spikes. A push-out then keeps every vertex at least
  `margin` metres outside the body.
- **Loose items** (the skirt) use a Gaussian radial-basis displacement of a
  few dozen nearby body vertices, blended toward the surface fit by tightness.
  They are not shrink-wrapped onto the legs.
- **Hold** (`rigid`) does not refit. The mesh only skins. New OxyBrush pieces
  whose median distance to the body is under 4 cm are fitted; farther pieces
  stay rigid, which is how a prop is told from a garment.
- **Weights** are the barycentric blend of the bound triangle, smoothed across
  garment edges, then the strongest four bones. Rigid accessories keep the
  older nearest-vertex transfer.
- **Hide-under.** A vertex is covered when the closest point on a garment lies
  outside it, within that garment's cover distance. A triangle is hidden only
  when all three vertices are covered. Items have a layer: a higher layer can
  hide triangles of a lower one the same way (shirt over trousers). Each
  garment still fits to the body, not to the garment under it. There is no
  collision between layers.
- **Content.** Four original meshes are generated from the body: a T-shirt and
  trousers (offset shells of rig regions, or of proportions when there is no
  rig), a flared skirt, and hair as ribbon cards over the scalp. No third-party
  garment is fetched or committed.
- **Packages** are a folder with `manifest.json` (`oxycharacter-wardrobe`,
  version 1) and Wavefront OBJ meshes. JSON matches `.oxyc`; OBJ is a format
  we already read. A zip container and glTF *import* are not part of this
  version.
- **Hair look.** A procedural alpha-tested strand texture plus a tangent-space
  normal that bends across each strand. The viewport draws that with alpha
  masking and no back-face culling. Export adds the same textures,
  `alphaMode: MASK`, `doubleSided`, and `KHR_materials_anisotropy` (used, not
  required) with the highlight running along the card. Bevy's standard
  material does not evaluate that extension, so the viewport highlight is the
  normal map only.
- **Projects.** Worn items (built-in id, or the embedded mesh and its binding),
  fit settings and the body triangles each item hides are optional fields on
  `.oxyc`. Older projects still open. Masks are recomputed when the body
  changes; the saved list is what was true at save time.

## Consequences

- Accessories that sit on the body now follow morphs. Props do not shrink-wrap.
- Changing the garment generator changes what a built-in id looks like. The
  project stores the id and the settings, not a frozen copy of a built-in mesh.
- Layering hides poke-through. It does not simulate a shirt draping over a
  jacket.
