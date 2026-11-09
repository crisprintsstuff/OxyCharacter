extends SceneTree

## Import an OxyCharacter .glb with Godot's glTF importer and check it.
##
##   godot --headless --path <this dir> -s res://check.gd -- file.glb MODE
##
## Modes: static, rest, tpose, blend, accessory.
## Prints `OXY_GLTF_REPORT {json}`. Original script; Godot is a test tool only.

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() != 2:
		quit(_finish(false, {"error": "usage: check.gd -- file.glb MODE", "args": str(args)}))
		return
	quit(_run(args[0], args[1]))


func _run(path: String, mode: String) -> int:
	if not mode in ["static", "rest", "tpose", "blend", "accessory"]:
		return _finish(false, {"error": "unknown mode %s" % mode})
	var doc := GLTFDocument.new()
	var state := GLTFState.new()
	var err := doc.append_from_file(path, state)
	if err != OK:
		return _finish(false, {"error": "glTF import failed (%s)" % error_string(err), "file": path, "mode": mode})
	var scene := doc.generate_scene(state)
	if scene == null:
		return _finish(false, {"error": "generate_scene returned null", "file": path, "mode": mode})
	root.add_child(scene)

	var body := _find(scene, "body") as MeshInstance3D
	if body == null or body.mesh == null:
		return _finish(false, {"error": "no body mesh", "file": path, "objects": _names(scene)})
	var report := {
		"file": path,
		"mode": mode,
		"vertices": body.mesh.surface_get_array_len(0) if body.mesh.get_surface_count() > 0 else 0,
		"objects": _names(scene),
	}
	if int(report["vertices"]) < 100:
		return _finish(false, report.merged({"error": "body has too few vertices"}))

	var skel := _find_skeleton(scene)
	if mode == "static":
		if skel != null:
			return _finish(false, report.merged({"error": "static export still has a skeleton"}))
		var tex := _albedo_size(body)
		report["albedo"] = {"width": tex.x, "height": tex.y}
		if tex.x < 8 or tex.y < 8:
			return _finish(false, report.merged({"error": "static export has no albedo texture"}))
		return _finish(true, report)

	if skel == null:
		return _finish(false, report.merged({"error": "no skeleton"}))
	report["bones"] = skel.get_bone_count()
	if skel.get_bone_count() <= 100:
		return _finish(false, report.merged({"error": "skeleton is too small"}))
	if body.skin == null:
		return _finish(false, report.merged({"error": "body has no skin"}))

	var rest_err := _rest_error(body, skel)
	report["rest_error_m"] = rest_err
	if rest_err < 0.0:
		return _finish(false, report.merged({"error": "could not read skin weights"}))
	if rest_err > 0.001:
		return _finish(false, report.merged({"error": "bind-pose skinning does not reproduce the mesh"}))

	if mode == "rest":
		var tex := _albedo_size(body)
		report["albedo"] = {"width": tex.x, "height": tex.y}
		if tex.x < 8 or tex.y < 8:
			return _finish(false, report.merged({"error": "textured body has no albedo texture"}))
		var normal := _has_normal(body)
		report["has_normal_map"] = normal
		if not normal:
			return _finish(false, report.merged({"error": "body has no normal map"}))
		for eye_name in ["eye.L", "eye.R"]:
			var eye := _find(scene, eye_name) as MeshInstance3D
			if eye == null or eye.skin == null:
				return _finish(false, report.merged({"error": "missing skinned %s" % eye_name}))
			var eye_err := _rest_error(eye, skel)
			report["%s_rest_error_m" % eye_name] = eye_err
			if eye_err < 0.0 or eye_err > 0.001:
				return _finish(false, report.merged({"error": "%s leaves its bind pose" % eye_name}))
			var eye_tex := _albedo_size(eye)
			report["%s_albedo" % eye_name] = {"width": eye_tex.x, "height": eye_tex.y}
			if eye_tex.x < 8:
				return _finish(false, report.merged({"error": "%s has no iris texture" % eye_name}))
		var moved := _deform(body, skel, "upperarm01.L", 0.8)
		report["arm_deform_m"] = moved
		if moved < 0.02:
			return _finish(false, report.merged({"error": "rotating the upper arm does not deform the body", "sample_bones": _bone_sample(skel)}))
		var eye := _find(scene, "eye.L") as MeshInstance3D
		var eye_moved := _deform(eye, skel, "eye.L", 0.5)
		report["eye_deform_m"] = eye_moved
		if eye_moved < 0.001:
			return _finish(false, report.merged({"error": "rotating the eye bone does not move the eyeball"}))
		return _finish(true, report)

	if mode == "tpose":
		var sh := _bone_origin(skel, "upperarm01.L")
		var wr := _bone_origin(skel, "wrist.L")
		if sh == Vector3.INF or wr == Vector3.INF:
			return _finish(false, report.merged({"error": "missing arm bones", "sample_bones": _bone_sample(skel)}))
		var ext := _extent(body)
		# Godot keeps glTF's Y-up. Arm span is about as tall as the body, so
		# the longest bounding-box side is not a reliable up axis.
		var delta := wr - sh
		report["up_axis"] = "y"
		report["shoulder"] = [sh.x, sh.y, sh.z]
		report["wrist"] = [wr.x, wr.y, wr.z]
		report["body_extent"] = [ext.x, ext.y, ext.z]
		if absf(delta.y) > 0.05:
			return _finish(false, report.merged({"error": "T-pose wrist is not level with the shoulder"}))
		if wr.x < sh.x + 0.3:
			return _finish(false, report.merged({"error": "T-pose left arm is not extended toward +X"}))
		return _finish(true, report)

	if mode == "blend":
		var mesh := body.mesh
		var names: Array = []
		for i in mesh.get_blend_shape_count():
			names.append(mesh.get_blend_shape_name(i))
		report["shape_keys"] = names
		var idx := names.find("Nose bump")
		if idx < 0:
			return _finish(false, report.merged({"error": "missing Nose bump shape key"}))
		var delta := _blend_delta(mesh, idx)
		report["shape_delta_m"] = delta
		if delta < 0.0001:
			return _finish(false, report.merged({"error": "shape key does not move any vertex"}))
		return _finish(true, report)

	var hat := _find(scene, "hat") as MeshInstance3D
	if hat == null or hat.skin == null:
		return _finish(false, report.merged({"error": "missing skinned hat accessory"}))
	var bone_name := _dominant_bone(hat, skel)
	report["hat_bone"] = bone_name
	if bone_name == "":
		return _finish(false, report.merged({"error": "hat vertex has no dominant weight"}))
	var moved := _deform(hat, skel, bone_name, 0.8)
	report["hat_deform_m"] = moved
	if moved < 0.0001:
		return _finish(false, report.merged({"error": "posing the hat's bone does not move it"}))
	var body_moved := _deform(body, skel, "upperarm01.L", 0.8)
	report["arm_deform_m"] = body_moved
	if body_moved < 0.02:
		return _finish(false, report.merged({"error": "accessory file body does not deform"}))
	return _finish(true, report)


func _finish(ok: bool, info: Dictionary) -> int:
	info["ok"] = ok
	info["engine"] = "godot"
	info["godot"] = Engine.get_version_info()["string"]
	print("OXY_GLTF_REPORT " + JSON.stringify(info))
	return 0 if ok else 1


func _find(node: Node, name: String) -> Node:
	if node.name == name or str(node.name).replace(".", "_") == name.replace(".", "_"):
		return node
	for child in node.get_children():
		var found := _find(child, name)
		if found != null:
			return found
	return null


func _find_skeleton(node: Node) -> Skeleton3D:
	if node is Skeleton3D:
		return node
	for child in node.get_children():
		var found := _find_skeleton(child)
		if found != null:
			return found
	return null


func _names(node: Node, out: Array = []) -> Array:
	out.append(str(node.name))
	for child in node.get_children():
		_names(child, out)
	return out


func _bone_index(skel: Skeleton3D, name: String) -> int:
	var idx := skel.find_bone(name)
	if idx >= 0:
		return idx
	return skel.find_bone(name.replace(".", "_"))


func _bone_sample(skel: Skeleton3D) -> Array:
	var names: Array = []
	for i in mini(12, skel.get_bone_count()):
		names.append(skel.get_bone_name(i))
	return names


## World (skeleton-space) bone transforms from the local poses.
## Godot 4's get_bone_global_pose() stays on the rest pose until the skeleton
## is processed for a frame, which a headless script that quits immediately
## never does. parent_global * bone_pose matches that API at rest and follows
## set_bone_pose_rotation.
func _bone_globals(skel: Skeleton3D) -> Array:
	var count := skel.get_bone_count()
	var out: Array = []
	out.resize(count)
	var done: Array = []
	done.resize(count)
	done.fill(false)
	for i in count:
		_fill_bone(skel, i, out, done)
	return out


func _fill_bone(skel: Skeleton3D, bone: int, out: Array, done: Array) -> void:
	if done[bone]:
		return
	var parent := skel.get_bone_parent(bone)
	var parent_global := Transform3D.IDENTITY
	if parent >= 0:
		_fill_bone(skel, parent, out, done)
		parent_global = out[parent]
	out[bone] = parent_global * skel.get_bone_pose(bone)
	done[bone] = true


func _bind_matrix(mi: MeshInstance3D, globals: Array, bind: int) -> Transform3D:
	# glTF JOINTS_0 indexes the skin's binds. Godot may map those binds onto
	# a different Skeleton3D bone order, so the skeleton bone comes from the skin.
	var skin: Skin = mi.skin
	var bone := skin.get_bind_bone(bind)
	if bone < 0 or bone >= globals.size():
		bone = bind if bind < globals.size() else 0
	return globals[bone] * skin.get_bind_pose(bind)


func _rest_error(mi: MeshInstance3D, skel: Skeleton3D) -> float:
	var arrays: Array = mi.mesh.surface_get_arrays(0)
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var bones = arrays[Mesh.ARRAY_BONES]
	var weights = arrays[Mesh.ARRAY_WEIGHTS]
	if bones == null or weights == null or verts.is_empty():
		return -1.0
	var globals := _bone_globals(skel)
	var worst := 0.0
	for i in verts.size():
		var acc := Vector3.ZERO
		var sum := 0.0
		for k in 4:
			var w: float = weights[i * 4 + k]
			if w <= 0.0:
				continue
			sum += w
			var b: int = bones[i * 4 + k]
			acc += _bind_matrix(mi, globals, b) * verts[i] * w
		if sum <= 0.0:
			return -1.0
		worst = maxf(worst, acc.distance_to(verts[i]))
	return worst


func _deform(mi: MeshInstance3D, skel: Skeleton3D, bone_name: String, angle: float) -> float:
	var idx := _bone_index(skel, bone_name)
	if idx < 0:
		return -1.0
	var before := _skinned(mi, skel)
	skel.set_bone_pose_rotation(idx, Quaternion(Vector3.RIGHT, angle))
	var after := _skinned(mi, skel)
	skel.reset_bone_pose(idx)
	return _max_delta(before, after)


func _skinned(mi: MeshInstance3D, skel: Skeleton3D) -> PackedVector3Array:
	var arrays: Array = mi.mesh.surface_get_arrays(0)
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var bones = arrays[Mesh.ARRAY_BONES]
	var weights = arrays[Mesh.ARRAY_WEIGHTS]
	var globals := _bone_globals(skel)
	var out := PackedVector3Array()
	out.resize(verts.size())
	for i in verts.size():
		var acc := Vector3.ZERO
		for k in 4:
			var w: float = weights[i * 4 + k]
			if w <= 0.0:
				continue
			var b: int = bones[i * 4 + k]
			acc += _bind_matrix(mi, globals, b) * verts[i] * w
		out[i] = acc
	return out


func _max_delta(a: PackedVector3Array, b: PackedVector3Array) -> float:
	if a.size() != b.size() or a.is_empty():
		return -1.0
	var worst := 0.0
	for i in a.size():
		worst = maxf(worst, a[i].distance_to(b[i]))
	return worst


func _dominant_bone(mi: MeshInstance3D, skel: Skeleton3D) -> String:
	var arrays: Array = mi.mesh.surface_get_arrays(0)
	var bones = arrays[Mesh.ARRAY_BONES]
	var weights = arrays[Mesh.ARRAY_WEIGHTS]
	if bones == null:
		return ""
	var best_i := 0
	var best_w := -1.0
	for k in 4:
		var w: float = weights[k]
		if w > best_w:
			best_w = w
			best_i = k
	if best_w < 0.5:
		return ""
	var bind: int = bones[best_i]
	var skin: Skin = mi.skin
	if bind < skin.get_bind_count():
		var bone := skin.get_bind_bone(bind)
		if bone >= 0 and bone < skel.get_bone_count():
			return skel.get_bone_name(bone)
	return ""


func _bone_origin(skel: Skeleton3D, name: String) -> Vector3:
	var idx := _bone_index(skel, name)
	if idx < 0:
		return Vector3.INF
	return skel.get_bone_global_rest(idx).origin


func _extent(mi: MeshInstance3D) -> Vector3:
	var verts: PackedVector3Array = mi.mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	var lo := verts[0]
	var hi := verts[0]
	for v in verts:
		lo = lo.min(v)
		hi = hi.max(v)
	return hi - lo


func _active_material(mi: MeshInstance3D) -> Material:
	var mat := mi.get_active_material(0)
	if mat != null:
		return mat
	if mi.mesh != null:
		return mi.mesh.surface_get_material(0)
	return null


func _albedo_size(mi: MeshInstance3D) -> Vector2i:
	var mat := _active_material(mi)
	if mat is StandardMaterial3D:
		var tex: Texture2D = (mat as StandardMaterial3D).albedo_texture
		if tex != null:
			return tex.get_size()
	if mat is ORMMaterial3D:
		var tex2: Texture2D = (mat as ORMMaterial3D).albedo_texture
		if tex2 != null:
			return tex2.get_size()
	return Vector2i.ZERO


func _has_normal(mi: MeshInstance3D) -> bool:
	var mat := _active_material(mi)
	if mat is StandardMaterial3D:
		return (mat as StandardMaterial3D).normal_texture != null
	if mat is ORMMaterial3D:
		return (mat as ORMMaterial3D).normal_texture != null
	return false


func _blend_delta(mesh: Mesh, shape: int) -> float:
	var base: PackedVector3Array = mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	var shapes: Array = mesh.surface_get_blend_shape_arrays(0)
	if shape >= shapes.size():
		return -1.0
	var block = shapes[shape]
	# Godot stores either a PackedVector3Array of positions or a full surface array.
	var pos: PackedVector3Array
	if block is PackedVector3Array:
		pos = block
	elif block is Array and block.size() > Mesh.ARRAY_VERTEX:
		pos = block[Mesh.ARRAY_VERTEX]
	else:
		return -1.0
	if pos.size() != base.size():
		return -1.0
	var worst := 0.0
	for i in base.size():
		worst = maxf(worst, base[i].distance_to(pos[i]))
	return worst
