# OxyCharacter Python SDK

Pure-Python plugin SDK for OxyCharacter. Plugins are scripts that the editor
(**Plugins** menu) or the CLI (`oxy plugin`) starts; they talk to their host
over stdin/stdout, so there are no ports or network access involved.

```python
"""
# Name: Athletic build
# Description: Make the character tall and muscular, then report the height
"""
import oxycharacter

app = oxycharacter.attach()
app.set_params(gender=1.0, muscle=0.9, weight=0.45, height=0.6)
app.notify(f"Height: {app.measure()['height']:.2f} m")
```

The editor and CLI put this folder on `PYTHONPATH` automatically. Full API and
a porting guide: [docs/PLUGINS.md](../docs/PLUGINS.md).
