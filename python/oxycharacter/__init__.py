"""OxyCharacter plugin SDK (pure Python, no dependencies).

A plugin is a Python script the editor (Plugins menu) or the CLI
(`oxy plugin`) starts. It talks to its host over stdin/stdout::

    import oxycharacter
    app = oxycharacter.attach()
    app.set_params({"gender": 1.0, "muscle": 0.8})
    app.notify("Done!")

See docs/PLUGINS.md for the full API.
"""

from .client import API_VERSION, App, PluginError, attach

__all__ = ["API_VERSION", "App", "PluginError", "attach"]
