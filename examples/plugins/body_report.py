"""
# Name: Body report
# Description: Print height, arm span and leg length of the current character
"""
import math

import oxycharacter

app = oxycharacter.attach()
m = app.measure()
bones = app.joints()


def dist(a, b):
    return math.dist(a, b)


def chain(names):
    return sum(dist(bones[n]["head"], bones[n]["tail"]) for n in names if n in bones)


arm = ["upperarm01.L", "upperarm02.L", "lowerarm01.L", "lowerarm02.L"]
leg = ["upperleg01.L", "upperleg02.L", "lowerleg01.L", "lowerleg02.L"]
print(f"height    {m['height']:.3f} m")
print(f"width     {m['width']:.3f} m (A-pose)")
print(f"arm       {chain(arm):.3f} m (shoulder to wrist)")
print(f"leg       {chain(leg):.3f} m (hip to ankle)")
if bones:
    ratio = chain(leg) / m["height"]
    app.notify(f"Height {m['height']:.2f} m · leg/height {ratio:.2f}")
else:
    app.notify(f"Height {m['height']:.2f} m (no skeleton in this bundle)")
