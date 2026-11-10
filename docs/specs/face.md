# Face and expressions (spec)

What OxyCharacter's face features do, in our own words.

## Expressions

- A face's expression is a set of **unit** weights from 0 to 1. Each unit is
  one movement: raise a brow, close an eye, open the jaw, pull the mouth
  corners up, pucker the lips, flare a nostril, tense the neck. Units are
  grouped Brows, Eyes, Nose, Mouth, Neck; left and right are separate where
  the face has two sides.
- **Presets** set several units at once (smile, big smile, sad, angry,
  surprise, fear, disgust, wink, blink, kiss). Picking one replaces the
  current units; the unit sliders then fine-tune it.
- **Mouth shapes (visemes)** are presets for speech sounds: A, E, I, O, U,
  M B P, F V, L, S Z, CH SH, TH. They are meant for lip sync in an engine.
- Expressions follow the face. Sliders that reshape the head (wider jaw,
  bigger eyes, older face) change the shape the expression is applied to;
  the expression is carried over so a smile still pulls the corners of
  *this* mouth.
- **Gaze** turns both eyes together, left/right up to 35° and up/down up to
  25°.

## Where it shows up

- Editor: a **Face** mode next to Morph. The upper card lists Neutral,
  presets and mouth shapes; the lower card has gaze and the unit sliders by
  group. Hovering a unit highlights the skin it moves. Changes are undoable
  and saved in the project.
- Export: the face exports neutral with every unit, preset and mouth shape as
  a named blendshape (`expr.*`, `face.*`, `viseme.*`), the units at their
  current weights, so the engine shows the same face and can animate it.
  Alternatively the expression is baked into the mesh.
- Plugins and the CLI can read and set expressions by id.

## Not yet

Teeth, tongue, mouth interior, eyelashes and brows as geometry; animated
expressions and lip sync; eye wetness. Gaze is viewport-only.
