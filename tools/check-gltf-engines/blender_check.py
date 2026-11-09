#!/usr/bin/env python3
"""Import an OxyCharacter .glb with Blender's glTF importer and check it.

Run via Blender, not a system python:

    blender --background --python blender_check.py -- file.glb rest

Modes: static, rest, tpose, blend, accessory.

Prints one line `OXY_GLTF_REPORT {json}` and exits 0 or 1. This script is
original; it only drives Blender's public Python API as a test tool.
"""

import json
import os
import sys

import bpy
from mathutils import Quaternion, Vector


def finish(ok, **info):
    info["ok"] = bool(ok)
    info["engine"] = "blender"
    info["blender"] = bpy.app.version_string
    print("OXY_GLTF_REPORT " + json.dumps(info), flush=True)
    os._exit(0 if ok else 1)


def argv_after_dd():
    if "--" not in sys.argv:
        finish(False, error="usage: blender --background --python blender_check.py -- file.glb MODE")
    args = sys.argv[sys.argv.index("--") + 1 :]
    if len(args) != 2:
        finish(False, error="expected FILE.glb MODE", args=args)
    return args[0], args[1]


def find_named(name):
    obj = bpy.data.objects.get(name)
    if obj is not None:
        return obj
    want = name.replace(".", "_")
    for candidate in bpy.data.objects:
        if candidate.name.replace(".", "_") == want:
            return candidate
    return None


def armature_of(obj):
    if obj is None:
        return None
    for mod in obj.modifiers:
        if mod.type == "ARMATURE" and mod.object is not None:
            return mod.object
    if obj.parent is not None and obj.parent.type == "ARMATURE":
        return obj.parent
    return None


def evaluated_positions(obj):
    deps = bpy.context.evaluated_depsgraph_get()
    ev = obj.evaluated_get(deps)
    mesh = ev.to_mesh()
    try:
        return [v.co.copy() for v in mesh.vertices]
    finally:
        ev.to_mesh_clear()


def max_distance(a, b):
    if len(a) != len(b) or not a:
        return None
    return max((p - q).length for p, q in zip(a, b))


def rest_error(obj):
    basis = [v.co.copy() for v in obj.data.vertices]
    posed = evaluated_positions(obj)
    err = max_distance(basis, posed)
    if err is None:
        finish(False, error="evaluated vertex count differs from the basis mesh", object=obj.name)
    return err


def weight_stats(obj):
    worst = 0.0
    empty = 0
    for v in obj.data.vertices:
        if not v.groups:
            empty += 1
            continue
        total = sum(g.weight for g in v.groups)
        worst = max(worst, abs(total - 1.0))
    return worst, empty


def image_report(obj):
    found = []
    has_normal = False
    if obj.data is None or not hasattr(obj.data, "materials"):
        return found, has_normal
    for mat in obj.data.materials:
        if mat is None or mat.node_tree is None:
            continue
        for node in mat.node_tree.nodes:
            if node.type == "NORMAL_MAP":
                has_normal = True
            if node.type == "TEX_IMAGE" and node.image is not None:
                image = node.image
                found.append(
                    {
                        "name": image.name,
                        "width": int(image.size[0]),
                        "height": int(image.size[1]),
                    }
                )
                if "normal" in image.name.lower():
                    has_normal = True
    return found, has_normal


def world_bounds(obj):
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def bone_world_head(arm, bone_name):
    bone = arm.data.bones.get(bone_name)
    if bone is None:
        alt = bone_name.replace(".", "_")
        bone = arm.data.bones.get(alt)
    if bone is None:
        return None
    return arm.matrix_world @ bone.head_local


def pose_bone(arm, bone_name):
    bone = arm.pose.bones.get(bone_name)
    if bone is None:
        bone = arm.pose.bones.get(bone_name.replace(".", "_"))
    return bone


def deform(obj, arm, bone_name, angle=0.8):
    before = evaluated_positions(obj)
    bone = pose_bone(arm, bone_name)
    if bone is None:
        names = [b.name for b in arm.data.bones[:12]]
        finish(False, error=f"missing bone {bone_name}", sample_bones=names, object=obj.name)
    bone.rotation_mode = "QUATERNION"
    bone.rotation_quaternion = Quaternion((1.0, 0.0, 0.0), angle)
    bpy.context.view_layer.update()
    after = evaluated_positions(obj)
    # Put the bone back so later checks see the bind pose.
    bone.rotation_quaternion = Quaternion((1.0, 0.0, 0.0), 0.0)
    bpy.context.view_layer.update()
    moved = max_distance(before, after)
    if moved is None:
        finish(False, error="pose evaluation changed the vertex count", object=obj.name)
    return moved


def require(cond, **info):
    if not cond:
        finish(False, **info)


def main():
    path, mode = argv_after_dd()
    if mode not in {"static", "rest", "tpose", "blend", "accessory"}:
        finish(False, error=f"unknown mode {mode}")
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    try:
        bpy.ops.import_scene.gltf(filepath=path)
    except Exception as exc:  # importer raises on a broken file
        finish(False, error=f"glTF import failed: {exc}", file=path, mode=mode)
    # Background Blender keeps the startup primitive on some installs. Drop it
    # so the report lists only what the file imported.
    for obj in list(bpy.data.objects):
        if obj.name == "Icosphere" or obj.name.startswith("Icosphere."):
            bpy.data.objects.remove(obj, do_unlink=True)

    body = find_named("body")
    require(body is not None and body.type == "MESH", error="no body mesh", file=path, mode=mode)
    report = {
        "file": path,
        "mode": mode,
        "vertices": len(body.data.vertices),
        "polygons": len(body.data.polygons),
        "objects": sorted(o.name for o in bpy.data.objects),
    }
    require(report["vertices"] > 100, error="body has too few vertices", **report)

    images, has_normal = image_report(body)
    report["images"] = images
    report["has_normal_map"] = has_normal

    armatures = [o for o in bpy.data.objects if o.type == "ARMATURE"]
    if mode == "static":
        require(not armatures, error="static export still has an armature", **report)
        require(len(images) >= 1, error="static export has no texture", **report)
        require(all(i["width"] >= 8 and i["height"] >= 8 for i in images), error="texture is empty", **report)
        finish(True, **report)

    require(len(armatures) == 1, error="expected one armature", **report)
    arm = armatures[0]
    report["bones"] = len(arm.data.bones)
    require(report["bones"] > 100, error="skeleton is too small", **report)
    require(armature_of(body) == arm, error="body is not skinned to the armature", **report)

    worst, empty = weight_stats(body)
    report["weight_error"] = worst
    report["unweighted"] = empty
    require(empty == 0, error="body has unweighted vertices", **report)
    require(worst < 1e-3, error="skin weights do not sum to 1", **report)

    err = rest_error(body)
    report["rest_error_m"] = err
    require(err < 1e-3, error="bind-pose skinning does not reproduce the mesh", **report)

    if mode == "rest":
        require(len(images) >= 3, error="textured body is missing maps", **report)
        require(has_normal, error="body has no normal map", **report)
        require(all(i["width"] >= 8 and i["height"] >= 8 for i in images), error="texture is empty", **report)
        for eye_name in ("eye.L", "eye.R"):
            eye = find_named(eye_name)
            require(eye is not None and eye.type == "MESH", error=f"missing {eye_name}", **report)
            require(armature_of(eye) == arm, error=f"{eye_name} is not skinned", **report)
            eye_err = rest_error(eye)
            report[f"{eye_name}_rest_error_m"] = eye_err
            require(eye_err < 1e-3, error=f"{eye_name} leaves its bind pose", **report)
            eye_images, _ = image_report(eye)
            report[f"{eye_name}_images"] = eye_images
            require(len(eye_images) >= 1, error=f"{eye_name} has no iris texture", **report)
        moved = deform(body, arm, "upperarm01.L")
        report["arm_deform_m"] = moved
        require(moved > 0.02, error="rotating the upper arm does not deform the body", **report)
        eye = find_named("eye.L")
        eye_moved = deform(eye, arm, "eye.L", angle=0.5)
        report["eye_deform_m"] = eye_moved
        require(eye_moved > 0.001, error="rotating the eye bone does not move the eyeball", **report)
        finish(True, **report)

    if mode == "tpose":
        shoulder = bone_world_head(arm, "upperarm01.L")
        wrist = bone_world_head(arm, "wrist.L")
        require(shoulder is not None and wrist is not None, error="missing arm bones", **report)
        lo, hi = world_bounds(body)
        extent = hi - lo
        # The importer turns glTF Y-up into Blender Z-up. A T-pose's arm span
        # is about as large as the body's height, so the up axis is not the
        # longest side of the bounding box.
        delta = wrist - shoulder
        report["up_axis"] = "z"
        report["shoulder"] = [shoulder.x, shoulder.y, shoulder.z]
        report["wrist"] = [wrist.x, wrist.y, wrist.z]
        report["body_extent"] = [extent.x, extent.y, extent.z]
        require(abs(delta.z) < 0.05, error="T-pose wrist is not level with the shoulder", **report)
        require(wrist.x > shoulder.x + 0.3, error="T-pose left arm does not point to +X", **report)
        finish(True, **report)

    if mode == "blend":
        keys = body.data.shape_keys
        names = [] if keys is None else [k.name for k in keys.key_blocks]
        report["shape_keys"] = names
        require(keys is not None and "Nose bump" in keys.key_blocks, error="missing Nose bump shape key", **report)
        block = keys.key_blocks["Nose bump"]
        delta = max(
            (block.data[i].co - body.data.vertices[i].co).length for i in range(len(body.data.vertices))
        )
        report["shape_delta_m"] = delta
        require(delta > 1e-4, error="shape key does not move any vertex", **report)
        block.value = 1.0
        bpy.context.view_layer.update()
        live = max_distance([v.co.copy() for v in body.data.vertices], evaluated_positions(body))
        report["shape_evaluated_m"] = live
        require(live is not None and live > 1e-4, error="shape key does not affect the evaluated mesh", **report)
        finish(True, **report)

    # accessory
    hat = find_named("hat")
    require(hat is not None and hat.type == "MESH", error="missing hat accessory", **report)
    require(armature_of(hat) == arm, error="hat is not skinned to the armature", **report)
    require(len(hat.data.vertices) >= 3 and len(hat.vertex_groups) >= 1, error="hat has no weights", **report)
    best = max(hat.data.vertices[0].groups, key=lambda g: g.weight)
    bone_name = hat.vertex_groups[best.group].name
    report["hat_bone"] = bone_name
    report["hat_weight"] = best.weight
    require(best.weight > 0.5, error="hat vertex has no dominant weight", **report)
    moved = deform(hat, arm, bone_name)
    report["hat_deform_m"] = moved
    require(moved > 1e-4, error="posing the hat's bone does not move it", **report)
    body_moved = deform(body, arm, "upperarm01.L")
    report["arm_deform_m"] = body_moved
    require(body_moved > 0.02, error="accessory file body does not deform", **report)
    finish(True, **report)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:
        finish(False, error=f"{type(exc).__name__}: {exc}")
