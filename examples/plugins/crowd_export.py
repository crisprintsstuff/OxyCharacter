"""
# Name: Crowd export
# Description: Export a handful of varied, rigged characters as .glb (good for headless runs)

Usage (headless):  oxy plugin assets/generated/makehuman.oxyb examples/plugins/crowd_export.py -- OUT_DIR [COUNT]
"""
import os
import random
import sys

import oxycharacter

app = oxycharacter.attach()
out = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else "crowd")
count = int(sys.argv[2]) if len(sys.argv) > 2 else 4
os.makedirs(out, exist_ok=True)
rng = random.Random(7)
irises = [[0.20, 0.11, 0.05], [0.12, 0.25, 0.48], [0.12, 0.28, 0.12], [0.30, 0.33, 0.36]]
tones = [[0.86, 0.66, 0.56], [0.62, 0.40, 0.29], [0.48, 0.29, 0.19], [0.16, 0.09, 0.06]]

for n in range(count):
    app.reset()
    app.set_params(
        gender=rng.choice([0.05, 0.95]),
        age=rng.uniform(0.45, 0.8),
        muscle=rng.uniform(0.3, 0.8),
        weight=rng.uniform(0.3, 0.75),
        height=rng.uniform(-0.6, 0.6),
    )
    app.set_material(skin_tone=rng.choice(tones), eyes={"iris": rng.choice(irises)})
    path = os.path.join(out, f"person_{n + 1:02d}.glb")
    stats = app.export_glb(path, texture_size=512)
    print(f"{path}: {app.measure()['height']:.2f} m, {stats}")

app.notify(f"Exported {count} characters to {out}")
