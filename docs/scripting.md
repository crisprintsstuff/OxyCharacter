# Script plugins

A plugin is a text file you write, in the [Rhai](https://rhai.rs) language,
with the extension `.rhai`. OxyCharacter does not ship a fixed set of
plugins. You add one by adding a file. The functions below are the whole
host API: a script cannot do something that is not in this list, and it
cannot open files or use the network.

The same file runs in the editor and in `oxy script`.

## Where plugins live

The editor looks in a `plugins` folder under the user directory:

| How you run | Plugins folder |
|---|---|
| Source checkout (`cargo run` from the repo) | `user/plugins` |
| `OXY_USER_DIR` set | `$OXY_USER_DIR/plugins` |
| `OXY_PLUGINS` set | that directory, instead of the one above |
| Portable install (a `portable` file beside the program) | `<program folder>/user/plugins` |
| Installed app | the platform user folder's `plugins` (see the install notes) |

Only files whose name ends in `.rhai` are loaded, in file-name order. A file
that fails to compile is skipped; the status line shows the error. **Scripts
→ Reload** reads the folder again.

The Scripts menu lists actions. Loading a file to build that list uses a
throwaway copy of the character, so top-level statements do not change the
open character until you run the action (or pass the file to `--script` /
`oxy script`).

## How to run a file

```sh
oxy script path/to/makehuman.oxyb plugins/hero.rhai
oxy script path/to/makehuman.oxyb plugins/hero.rhai --call hero
oxycharacter --script plugins/hero.rhai
oxycharacter --script plugins/hero.rhai --call hero
oxycharacter --call hero
```

`--call` without `--script` runs that action from the plugins folder.

`oxy script` prints:

```text
log: <each log() line>
<id>=<value>          # one line per slider that is not at its default
pose=<preset name>
wearing=tee:0.8,hair  # omitted when nothing is worn; tightness only if set
action <id>	<label>   # one line per action() in the file
```

In the editor, a script you run is one undo step. `--script` on a fresh
launch is the starting document (the same way `--wear` is), so it is not
itself an undo step. `--script` together with `--open` is an edit on top of
the opened project.

## What a file should look like

Top-level statements run first. Then `on_load` runs, if you defined it.
Put character changes in functions. Call `action` at the top level (or in
`on_load`) so the Scripts menu can show the function.

```rhai
fn hero() {
    reset_all();
    set("gender", 1);
    set("muscle", 0.8);
    set_pose("Wave");
    wear("tee", 0.7);
    wear("pants");
    set_skin(0.55, 0.36, 0.26);
    log("hero");
}

action("hero", "Muscular hero");
```

`action`'s id must be the function's name: letters, digits and underscores,
not starting with a digit. Defining the function and calling `action` is
enough. You do not register a plugin type.

## Limits

- At most 1 000 000 operations. A `loop` that never ends is stopped. The
  error text contains `stopped`.
- No `import` of another file. There is no file resolver.
- No network.
- Numbers may be written as `1` or `1.0`. Values are clamped to the range
  given under each function.
- Colours are linear RGB, each channel from 0 to 1.
- A host error (unknown slider, unknown pose, garment not worn) aborts the
  script. Rhai's `try` / `catch` can catch it.
- This page does not restate the Rhai language. Functions, `if`, `for` and
  `loop` are the language's. The calls below are OxyCharacter's.

## Sliders

`set` clamps to the slider's minimum and maximum. While symmetry is on
(the default), `set` and `reset` also write the slider's mirror, when it
has one, to the same value. `set_symmetry(false)` writes only the id you
name.

| Call | Returns | Errors |
|---|---|---|
| `param_ids()` | Array of slider ids, catalog order | |
| `params()` | Array of maps, one per slider (same fields as `param`) | |
| `param(id)` | Map: `id`, `label`, `group` (array of strings), `min`, `max`, `default`, `value`, `mirror` (the other id, or `""`) | Unknown id |
| `get(id)` | Current value | Unknown id |
| `set(id, value)` | The clamped value | Unknown id |
| `reset(id)` | nothing | Unknown id. Sets the default (and the mirror, if symmetry is on) |
| `reset_all()` | nothing | Every slider returns to its default |
| `changed()` | Array of ids whose value is not the default | |
| `symmetry()` | `true` or `false` | |
| `set_symmetry(on)` | nothing | |
| `slider_count()` | How many sliders the base has | |

```rhai
let gender = param("gender");
set(gender.id, 1);
```

Ids are the catalog ids (`gender`, `muscle`, …), not the menu path. `param`
and `get` are how you discover an id's range.

## Pose

| Call | Returns | Errors |
|---|---|---|
| `poses()` | `["Rest (A-pose)", "T-pose", "Relaxed", "Wave", "Stride"]` | |
| `pose()` | The current preset's canonical name | |
| `set_pose(name)` | nothing | Unknown name. Matching ignores case; the stored name is the canonical one |

## Wardrobe

Built-in ids are `tee`, `pants`, `skirt` and `hair`. A script cannot add a
mesh. Pieces brought in from OxyBrush are left as they are when the editor
applies the script; the script's wear list replaces only these four ids.

| Call | Returns | Errors |
|---|---|---|
| `garments()` | `["tee", "pants", "skirt", "hair"]` | |
| `worn()` | Array of ids currently worn by the script | |
| `wear(id)` | nothing | Unknown id. Wearing it again keeps the previous tightness and colour |
| `wear(id, tightness)` | nothing | Unknown id. Tightness is clamped to 0..1. `0.5` is the shape the item was made at |
| `take_off(id)` | nothing | Unknown id. Taking off something that is not worn is fine |
| `set_tightness(id, value)` | nothing | The id is not worn, or it is not a built-in. Value clamped to 0..1 |
| `set_garment_color(id, r, g, b)` | nothing | The id is not worn. Channels clamped to 0..1. Until you call this, the editor keeps the built-in colour |

```rhai
wear("tee", 0.8);
set_garment_color("tee", 0.15, 0.25, 0.55);
take_off("skirt");
```

## Skin and eyes

`skin` is the body tone. The other colours are the freckle, blush, eye-shadow,
lip and iris colours. Amounts are 0..1 except where the clamp says otherwise.
`skin_seed` is an integer seed for the skin pattern.

| Get | Set | Clamp |
|---|---|---|
| `skin()` → `[r, g, b]` | `set_skin(r, g, b)` | each 0..1 |
| `variation()` | `set_variation(x)` | 0..1 |
| `detail()` | `set_detail(x)` | 0..1 |
| `roughness()` | `set_roughness(x)` | 0..1 |
| `freckles()` | `set_freckles(x)` | 0..1 |
| `freckle_color()` → `[r, g, b]` | `set_freckle_color(r, g, b)` | each 0..1 |
| `blush()` | `set_blush(x)` | 0..1 |
| `blush_color()` → `[r, g, b]` | `set_blush_color(r, g, b)` | each 0..1 |
| `eye_shadow()` | `set_eye_shadow(x)` | 0..1 |
| `eye_shadow_color()` → `[r, g, b]` | `set_eye_shadow_color(r, g, b)` | each 0..1 |
| `lips()` | `set_lips(x)` | 0..1 |
| `lip_color()` → `[r, g, b]` | `set_lip_color(r, g, b)` | each 0..1 |
| `skin_seed()` | `set_skin_seed(n)` | integer ≥ 0. A value above 4294967295 is stored as 4294967295 |
| `iris()` → `[r, g, b]` | `set_iris(r, g, b)` | each 0..1 |
| `iris_angle()` | `set_iris_angle(x)` | 0.05..1.5 radians |
| `pupil()` | `set_pupil(x)` | 0..1, as a fraction of the iris |
| `eye_size()` | `set_eye_size(x)` | 0.05..0.5, as a fraction of the distance between the eyes |

```rhai
set_skin(0.62, 0.40, 0.28);
set_freckles(0.4);
set_iris(0.15, 0.28, 0.55);
```

## Body info

| Call | Returns |
|---|---|
| `vertex_count()` | Base mesh vertex count |
| `bone_count()` | Bone count, or 0 when the base has no rig |
| `slider_count()` | `params().len()` |

## Log and actions

| Call | Returns | Errors |
|---|---|---|
| `log(message)` | nothing. The editor does not show these lines; `oxy script` prints `log: ` plus the text | |
| `action(id, label)` | nothing | `id` is empty, starts with a digit, or contains a character other than letters, digits and `_`. After the file loads, a function with that id and no parameters must exist |

`on_load` is not a host function. If your file defines `fn on_load() { ... }`,
the host calls it after the top-level statements, with no arguments. Use it
for registration. Do not require it.

## Example

`plugins/hero.rhai`:

```rhai
fn hero() {
    reset_all();
    set_symmetry(true);
    set("gender", 1);
    set("muscle", 0.85);
    set("weight", 0.55);
    set("height", 0.4);
    set_pose("wave");
    take_off("skirt");
    wear("tee", 0.65);
    wear("pants", 0.55);
    set_skin(0.45, 0.30, 0.22);
    set_roughness(0.5);
    log("hero applied");
}

action("hero", "Hero");
```

```sh
oxy script assets/generated/makehuman.oxyb plugins/hero.rhai --call hero
```

The editor shows **Hero** under Scripts. Choosing it runs `hero` and can be
undone with Ctrl+Z.
