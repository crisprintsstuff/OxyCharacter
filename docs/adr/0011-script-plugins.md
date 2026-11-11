# ADR 0011: Plugins are Rhai scripts against a fixed host

**Status:** accepted

## Context

People need to add their own character setups (a hero preset, a batch of
slider changes, a wardrobe combination) without a new editor build. The
commercial character-creator plugin formats are out of scope: this project
does not read or reimplement them ([CLEAN_ROOM.md](../CLEAN_ROOM.md)).

The editor and the CLI must share one meaning for a script. A plugin must
not open files or the network, matching the rule that only
`tools/fetch-assets.sh` downloads anything.

## Decision

- The language is [Rhai](https://rhai.rs) (MIT). A plugin is one `.rhai`
  file. There is no registry of plugin kinds: a new file that calls the host
  functions is a new plugin.
- The host functions are the list in [scripting.md](../scripting.md). That
  list is the API. A function that is not in the list is not available.
  `import` is not given a file resolver.
- A script runs against a `World` of slider values, pose name, built-in
  garments, skin and eyes. It does not touch the mesh, the renderer, or
  project files. The editor copies the open character into a `World`, runs
  the script, and writes the result back as one undo step. The CLI prints
  the values that changed.
- Top-level statements run when the file is loaded. `on_load`, if defined,
  runs after that. `action(id, label)` records that the function `id` should
  appear in the editor's Scripts menu. The menu re-runs the file and then
  calls that function.
- Plugins are read from `<user dir>/plugins`, or from `OXY_PLUGINS` when that
  is set. A script that fails to compile is skipped and reported; the others
  still load.
- A script that exceeds 1 000 000 operations is stopped.

## Consequences

- Adding a host function is a documentation change in the same commit as the
  code. The tests in `oxy-script` are the check that the documented calls
  work.
- Plugins cannot ship a mesh or read a project. Those stay on the editor
  commands that already exist.
- Rhai upgrades have to keep the host function names stable.
