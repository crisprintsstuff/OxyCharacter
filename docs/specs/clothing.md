# Clothing and hair

What the user can do. Design notes are in [ADR 0007](../adr/0007-clothing-and-hair.md).

- The **Wardrobe** tab lists clothes and hair that can be put on the character.
  Turning one on makes it follow the current body. Changing a body slider
  refits it. Changing the pose deforms it with the skeleton.
- Each worn item has a way of following the body:
  - **Fit** — stay a set distance off the skin.
  - **Loose** — follow the body's overall change without hugging every limb
    (used for the skirt).
  - **Hold** — do not refit; only move with the bones.
- **Tightness** pulls the item toward the skin or lets it stand off. The
  middle of the slider is the shape it was made at.
- **Hide body underneath** removes body triangles that sit fully under the
  item, so skin does not show through. An item on a higher layer can hide
  triangles of one on a lower layer where they overlap (a shirt over trousers).
- Built-in items are a T-shirt, trousers, a skirt and hair. Hair is a set of
  flat cards with a strand pattern you can see through. Its colour is one of
  the swatches.
- A **wardrobe folder** (`manifest.json` plus OBJ files) can be loaded from
  the tab or with `oxy export --wardrobe`. Pieces brought back from OxyBrush
  are fitted when they sit on the body, and left as rigid props when they do
  not. The choice can be changed per piece.
- Worn items, their settings and the hidden body triangles are saved in the
  project and undo with everything else.
- Export includes the worn items as skinned meshes next to the body. Hair
  keeps its strand texture. Hidden body triangles are left out of the body.
