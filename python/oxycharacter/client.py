"""Host connection and the API wrapper."""

import json
import os
import sys

API_VERSION = 1


class PluginError(RuntimeError):
    """The host refused a call (bad parameters, unknown method, failed action)."""

    def __init__(self, code, message):
        super().__init__(f"{message} ({code})")
        self.code = code
        self.message = message


class _Channel:
    """Content-Length framed JSON-RPC over the process's original stdio."""

    def __init__(self):
        if not os.environ.get("OXY_PLUGIN_API"):
            raise RuntimeError(
                "not running under OxyCharacter: start this script from the editor's "
                "Plugins menu or with `oxy plugin <bundle> <script.py>`"
            )
        # Keep the real stdout for the protocol, then point fd 1 (and print) at
        # stderr so stray output becomes log text instead of corrupting frames.
        self._out = os.fdopen(os.dup(1), "wb", buffering=0)
        os.dup2(2, 1)
        sys.stdout = sys.stderr
        self._in = sys.stdin.buffer
        self._next = 0

    def _read(self):
        length = None
        while True:
            line = self._in.readline()
            if not line:
                raise ConnectionError("the host closed the connection")
            line = line.strip()
            if not line:
                if length is not None:
                    break
                continue
            key, _, value = line.decode("ascii").partition(":")
            if key.strip().lower() == "content-length":
                length = int(value)
        return json.loads(self._in.read(length).decode("utf-8"))

    def _write(self, msg):
        body = json.dumps(msg, separators=(",", ":")).encode("utf-8")
        self._out.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)

    def call(self, method, **params):
        self._next += 1
        self._write({"jsonrpc": "2.0", "id": self._next, "method": method, "params": params})
        reply = self._read()
        if "error" in reply:
            err = reply["error"]
            raise PluginError(err.get("code"), err.get("message"))
        return reply.get("result")


class App:
    """The character being edited. Every method is one call to the host."""

    def __init__(self, channel):
        self._ch = channel
        self.info = channel.call("info")
        if self.info.get("api_version", 0) < API_VERSION:
            raise RuntimeError(f"host API {self.info.get('api_version')} is older than this SDK ({API_VERSION})")

    # -- raw access --------------------------------------------------------
    def call(self, method, **params):
        """Call any API method by name (see docs/PLUGINS.md)."""
        return self._ch.call(method, **params)

    # -- sliders ------------------------------------------------------------
    def params(self, group=None, search=None):
        """List sliders: dicts with id, label, group, min, max, default, value."""
        args = {}
        if group:
            args["group"] = group
        if search:
            args["search"] = search
        return self._ch.call("params.list", **args)

    def get(self, *ids):
        """Values of the given slider ids (all sliders when none are given)."""
        return self._ch.call("params.get", **({"ids": list(ids)} if ids else {}))

    def set_params(self, values=None, **kw):
        """Set sliders: `set_params({"gender": 1})` or `set_params(gender=1)`.
        Values are clamped; an unknown id raises and nothing changes."""
        values = dict(values or {}, **kw)
        return self._ch.call("params.set", values=values)

    def reset(self, *ids):
        return self._ch.call("params.reset", **({"ids": list(ids)} if ids else {}))

    # -- look ---------------------------------------------------------------
    def material(self):
        return self._ch.call("material.get")

    def set_material(self, skin_tone=None, skin=None, eyes=None):
        """Change skin/eye settings; only the fields you pass change.
        Colours are linear RGB lists, e.g. eyes={"iris": [0.1, 0.25, 0.5]}."""
        args = {}
        if skin_tone is not None:
            args["skin_tone"] = list(skin_tone)
        if skin:
            args["skin"] = skin
        if eyes:
            args["eyes"] = eyes
        return self._ch.call("material.set", **args)

    def poses(self):
        return self._ch.call("pose.list")

    def pose(self):
        return self._ch.call("pose.get")

    def set_pose(self, name):
        return self._ch.call("pose.set", name=name)

    # -- face ---------------------------------------------------------------
    def expressions(self):
        """Units this character has, presets, visemes and the gaze limit."""
        return self._ch.call("expression.list")

    def expression(self):
        """{"units": {id: weight}, "gaze": [yaw, pitch]} (degrees)."""
        return self._ch.call("expression.get")

    def set_expression(self, preset=None, viseme=None, units=None, gaze=None, replace=False):
        """Apply a preset and/or viseme, then unit weights (0..1), then gaze.
        With replace=True the other units are cleared first (gaze is kept)."""
        p = {"replace": replace}
        for k, v in (("preset", preset), ("viseme", viseme), ("units", units), ("gaze", gaze)):
            if v is not None:
                p[k] = list(v) if k == "gaze" else v
        return self._ch.call("expression.set", **p)

    # -- body ---------------------------------------------------------------
    def body_mesh(self, triangles=False, expression=False):
        """Rest-pose body: {"positions": [[x, y, z], ...], "triangles"?: [[a, b, c], ...]}.
        Metres, Y up, the character faces +Z and stands at y = 0. Neutral face
        unless expression=True."""
        return self._ch.call("body.mesh", triangles=triangles, expression=expression)

    def joints(self):
        """Bones with world-space head/tail positions, by name."""
        return {b["name"]: b for b in self._ch.call("body.joints")["bones"]}

    def measure(self):
        return self._ch.call("body.measure")

    def add_morph(self, label, positions=None, indices=None, deltas=None, value=1.0):
        """Create a sculpted-morph slider (Custom > Sculpted) and set it to `value`.
        Give `positions` for every body vertex (the new shape), or sparse
        `indices` + `deltas`. Returns the new slider id."""
        args = {"label": label, "value": value}
        if positions is not None:
            args["positions"] = positions
        else:
            args["indices"] = list(indices)
            args["deltas"] = [list(d) for d in deltas]
        return self._ch.call("morph.add", **args)["id"]

    # -- wardrobe -------------------------------------------------------------
    def wardrobe(self):
        return self._ch.call("wardrobe.list")

    def wear(self, items):
        """Put on items: "tee,pants:0.8,hair" (id or id:tightness) or a list."""
        if not isinstance(items, str):
            items = ",".join(items)
        return self._ch.call("wardrobe.wear", items=items)["worn"]

    def undress(self):
        return self._ch.call("wardrobe.clear")

    # -- files ----------------------------------------------------------------
    def export_glb(self, path, **options):
        """Export a rigged glTF. Options: tpose, blendshapes, textures,
        texture_size, eyes, skeleton."""
        return self._ch.call("export.glb", path=os.fspath(os.path.abspath(path)), **options)

    def save_project(self, path):
        return self._ch.call("project.save", path=os.fspath(os.path.abspath(path)))["path"]

    def notify(self, text):
        """Show a message to the user (status bar in the editor, stdout in the CLI)."""
        self._ch.call("notify", text=str(text))


_app = None


def attach():
    """Connect to the host that started this script (once per process)."""
    global _app
    if _app is None:
        _app = App(_Channel())
    return _app
