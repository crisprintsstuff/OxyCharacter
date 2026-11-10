# Plugins

Plugins are Python scripts that drive OxyCharacter: set sliders and materials,
generate sculpted morphs from the live mesh, dress and pose the character,
export, save projects. The same script runs in the editor and headless.

- **In the editor:** **Plugins** menu → pick a plugin, or *Run script…* for any
  `.py` file. Output and errors appear in the Plugins window (*Show log*). A
  whole run is **one undo step** ("Plugin: <name>").
- **Headless:** `oxy plugin <bundle.oxyb> <script.py> [--open project.oxyc] [--morphs DIR] [-- args…]`.
  Script arguments after `--` arrive in `sys.argv[1:]`.

Plugins are found in your user folder's `plugins/` (shown in the Plugins menu;
`user/plugins` in a checkout) and the bundled examples (`examples/plugins` in a
checkout, `plugins/` next to installed binaries). Files starting with `_` are
skipped, so helpers can live alongside.

> **Not compatible with other character creators' plugins.** This is
> OxyCharacter's own API (see [ADR 0011](adr/0011-plugin-api.md)). Plugins
> written for other products can't run here unchanged; the
> [porting guide](#porting-a-plugin-from-another-tool) below shows how to
> rewrite their logic.

## Requirements and safety

- **Python 3.8+** (`python3`, or set `OXY_PYTHON`). The SDK is pure Python with
  no dependencies; hosts put it on `PYTHONPATH` (`OXY_PYTHON_SDK` overrides).
- A plugin is a program **running with your rights**, like any script you run.
  Only run plugins you trust. OxyCharacter starts one only when you ask
  (menu, `--run-plugin`, `oxy plugin`).
- The connection is the plugin's **stdin/stdout**. There are no sockets or
  ports, and OxyCharacter itself makes no network calls. `print()` goes to the
  log, never into the protocol.

## A first plugin

```python
"""
# Name: Athletic build
# Description: Tall, muscular and lean; reports the new height
"""
import oxycharacter

app = oxycharacter.attach()
app.set_params(muscle=0.9, weight=0.42, height=0.55)
app.notify(f"{app.measure()['height']:.2f} m tall")
```

The `# Name:` and `# Description:` lines (in a docstring or comments, within
the first 40 lines) name the plugin in the menu. Save it to your `plugins/`
folder and use *Plugins → Rescan plugins*.

## SDK reference (`oxycharacter`)

`app = oxycharacter.attach()` connects once; `app.info` holds the host's
`info` reply. Every method below is one call; failures raise
`oxycharacter.PluginError` (`.code`, `.message`). `app.call(method, **params)`
reaches any method by name.

| Python | Method | Notes |
|---|---|---|
| `app.params(group=None, search=None)` | `params.list` | sliders: `id, label, group, min, max, default, value`; `group` is a prefix like `"Head/Nose"` |
| `app.get(*ids)` | `params.get` | `{id: value}`; all when no ids |
| `app.set_params(values=None, **kw)` | `params.set` | clamped to each slider's range; an unknown id raises and **nothing** is set |
| `app.reset(*ids)` | `params.reset` | all sliders when no ids |
| `app.material()` / `app.set_material(skin_tone=, skin=, eyes=)` | `material.get` / `material.set` | partial objects; colours linear RGB. Skin: `variation, detail, roughness, freckles, freckle_color, blush, blush_color, eye_shadow, eye_shadow_color, lips, lip_color, seed`. Eyes: `iris, iris_angle, pupil, size` |
| `app.poses()`, `app.pose()`, `app.set_pose(name)` | `pose.*` | preview pose presets |
| `app.expressions()` | `expression.list` | `units` (`id, label, group`) this character has, `presets` and `visemes` (`id, label, units`), `gaze_limit` `[yaw, pitch]` |
| `app.expression()` / `app.set_expression(preset=, viseme=, units={id: w}, gaze=[yaw, pitch], replace=False)` | `expression.get` / `expression.set` | weights 0–1; a preset or viseme sets the units it names, then `units`, then `gaze` (degrees, + = character's left / up). `replace` clears the other units first. Unknown ids raise and nothing is set |
| `app.body_mesh(triangles=False, expression=False)` | `body.mesh` | rest-pose body positions (metres, Y up, faces +Z, feet at y = 0) in base-vertex order; optional triangles; the neutral face unless `expression` |
| `app.joints()` | `body.joints` | `{bone: {name, parent, head, tail}}` |
| `app.measure()` | `body.measure` | `height, width, depth, min, max` |
| `app.add_morph(label, positions=…)` or `(label, indices=…, deltas=…)`, `value=1.0` | `morph.add` | new slider under *Custom › Sculpted*, set to `value`; returns its id. The skeleton follows it |
| `app.wardrobe()`, `app.wear("tee,pants:0.8,hair")`, `app.undress()` | `wardrobe.*` | built-in items; `id:tightness` |
| `app.export_glb(path, **opts)` | `export.glb` | rigged glTF; options `tpose, blendshapes, textures, texture_size, eyes, skeleton, expressions` |
| `app.save_project(path)` | `project.save` | `.oxyc` (in the editor this is a normal save) |
| `app.notify(text)` | `notify` | status bar in the editor; printed by the CLI |

`info` returns `api_version` (currently **1**), `host` (`"editor"` or
`"cli"`), the base mesh (`name, license, vertices, triangles, bones`) and the
document (`path, unsaved`).

Differences between hosts: the editor's wardrobe includes fitting and
rendering; headless `project.save` does not store worn items yet, and
headless hosts only know built-in garments.

## Protocol (for other languages)

JSON-RPC 2.0 over the child's stdio, framed like LSP:

```text
Content-Length: 57\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"params.get","params":{}}
```

The plugin sends requests; the host answers each with `result` or
`error {code, message}` (`-32601` unknown method, `-32602` bad params,
`-32000` the host couldn't do it). Requests without `id` are notifications
and get no reply. `OXY_PLUGIN_API` is set to the API version in the plugin's
environment. stderr is the plugin's log.

**Versioning:** new methods and new optional fields don't change
`api_version`; removing or changing the meaning of anything does, and hosts
keep answering the old version where they can.

## Porting a plugin from another tool

Plugins written for other character creators use those products' own
programming interfaces and file formats, which OxyCharacter doesn't
implement. Their **logic** usually ports directly. Rewrite each step against
the API above:

| What the old plugin does | Here |
|---|---|
| Sets morph or slider values on the character | `app.params()` to find ids (use `search=`), then `app.set_params({...})`. Values are slider units (0–1, or −1–1 for two-sided sliders) |
| Applies a body or face preset | a dict of slider values passed to `set_params`; ship it inside the plugin |
| Builds a new morph from vertex math | `app.body_mesh()` → compute new positions → `app.add_morph(label, positions=...)` |
| Changes skin or eye colour or appearance | `app.set_material(skin_tone=..., skin={...}, eyes={...})`. Layers here are parameters, not image files |
| Dresses the character or swaps clothing | `app.wear("tee,pants")`, `app.undress()` |
| Poses the character for a render or export | `app.set_pose("T-pose")`; export with `app.export_glb(path, tpose=True)` |
| Batch-exports characters | loop: `reset` → `set_params` → `export_glb` (see `examples/plugins/crowd_export.py`), headless via `oxy plugin` |
| Sets a facial expression or mouth shape | `app.set_expression(preset="smile")`, `app.set_expression(viseme="oh", replace=True)`; ids from `app.expressions()` |
| Reads bone positions | `app.joints()` |
| Shows a message | `app.notify(text)`; `print()` for the log |

Things with no equivalent yet: plugin-defined panels and dialogs, animation
timelines (keyframed expressions or lip sync), render settings.

## Examples

`examples/plugins/`:
- `athletic_build.py` — sliders + material;
- `body_report.py` — measurements and bones;
- `pointed_ears.py` — a procedural sculpted morph from the live mesh;
- `crowd_export.py` — headless batch export of varied characters.
