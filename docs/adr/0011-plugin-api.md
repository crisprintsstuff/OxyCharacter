# ADR 0011: Our own plugin API over stdio, not a compatibility layer

**Status:** accepted (2026-11-10)

## Context

The maintainer asked for "the ability to use CC plugins natively", meaning
plugins written for a commercial character creator. Those are either
compiled modules against that product's SDK and runtime, or Python scripts
using its proprietary scripting API and its content formats. Running them
unchanged would mean re-creating that API, its scene model and its formats,
which [CLEAN_ROOM.md](../CLEAN_ROOM.md) forbids and which would tie the project
to another vendor's licence terms. The maintainer chose our own plugin API
instead.

## Decision

- **Own API, Python first.** A JSON-RPC API (`oxy-plugin`, version 1) over
  operations OxyCharacter actually has: sliders, materials, poses, body
  geometry and joints, procedural sculpted morphs, wardrobe, export,
  projects, notifications. A pure-Python SDK (`python/oxycharacter`) wraps it.
  No names, call shapes or formats of another product's API are copied; a
  porting guide in [PLUGINS.md](../PLUGINS.md) maps *tasks*, not calls.
- **Transport: the child's stdio.** Hosts start the plugin process and talk
  `Content-Length`-framed JSON-RPC over its stdin/stdout. No sockets or ports,
  so "no network access from the editor or CLI" still holds. This differs from
  OxyBrush (loopback MCP server) on purpose; the SDK style (`attach()`,
  `# Name:` headers) matches it.
- **One dispatcher.** `oxy_plugin::dispatch` answers every method from a small
  `Host` trait; the editor (`EditorHost`) and the CLI (`SimpleHost`) implement
  the trait, so a plugin behaves the same in both.
- **Editor integration.** Requests are answered on the main thread, at most 64
  per frame. Undo snapshots are held back during a run and the run is
  committed as one step labelled with the plugin's name.
- **Trust model.** A plugin is a program the user chooses to run, with their
  rights. Hosts start one only on explicit request; the SDK redirects
  `print` to stderr (the log) so stray output can't corrupt the protocol.

## Consequences

- Plugins can't draw UI yet (panels, dialogs) or react to edits; a later API
  version can add notifications from the host and UI descriptions.
- Other languages can implement the protocol from PLUGINS.md.
- Release packages ship the SDK and example plugins next to the binaries.
