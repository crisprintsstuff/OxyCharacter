# ADR 0001: Engine-agnostic core crates, Bevy only in the app

**Status:** accepted

All character logic (morphs, rig, fitting, materials, IO) lives in crates that
depend only on `glam`/`serde`-level libraries. Bevy + egui is used solely in
`oxy-app`. This keeps the core headless-testable, usable from `oxy-cli`, and
protects us from renderer API churn.
