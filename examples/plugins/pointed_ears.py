"""
# Name: Pointed ears
# Description: Add a "Pointed ears" sculpted-morph slider, computed from the current head
"""
import oxycharacter

app = oxycharacter.attach()
mesh = app.body_mesh()
pos = mesh["positions"]

# Ears from the current shape (works for any head): in the head region, each
# ear's outermost point sticks out sideways; the ear is the cluster around it.
height = app.measure()["height"]
head = [i for i, p in enumerate(pos) if p[1] > height * 0.87]

indices, deltas = [], []
for s in (1.0, -1.0):
    side = [i for i in head if pos[i][0] * s > 0]
    tip = max(side, key=lambda i: abs(pos[i][0]))
    tx, ty, tz = pos[tip]
    ear = [
        i for i in side
        if abs(pos[i][0]) > abs(tx) - 0.022 and abs(pos[i][1] - ty) < 0.045 and abs(pos[i][2] - tz) < 0.035
    ]
    top = max(pos[i][1] for i in ear)
    bottom = min(pos[i][1] for i in ear)
    for i in ear:
        x, y, z = pos[i]
        t = (y - bottom) / max(top - bottom, 1e-6)  # 0 at the lobe, 1 at the top
        w = max(0.0, t - 0.5) / 0.5                 # only the upper half moves
        if w <= 0:
            continue
        indices.append(i)
        deltas.append([s * 0.008 * w * w, 0.020 * w * w, -0.010 * w * w])

slider = app.add_morph("Pointed ears", indices=indices, deltas=deltas, value=1.0)
app.notify(f"Added slider {slider} ({len(indices)} vertices)")
