"""
# Name: Athletic build
# Description: Tall, muscular and lean, with a light tan; reports the new height
"""
import oxycharacter

app = oxycharacter.attach()
app.set_params(muscle=0.9, weight=0.42, height=0.55)
app.set_material(skin={"variation": 0.4, "detail": 0.6})
app.notify(f"Athletic build applied: {app.measure()['height']:.2f} m tall")
