//! Scripting host for OxyCharacter plugins.
//!
//! A plugin is a [Rhai](https://rhai.rs) script. The host functions in this
//! crate are the whole API: anything a plugin can do is one of them, and
//! they are documented in `docs/scripting.md`. Scripts cannot open files or
//! the network. People add a plugin by writing another `.rhai` file; there
//! is no fixed list of plugin kinds.
//!
//! Top-level statements run when the file is loaded. Put character changes
//! in functions, and register those functions with `action` if the editor
//! should list them.

mod world;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, Scope, AST};

pub use world::{Action, ParamInfo, World, WornItem};

/// A script that keeps going past this many operations is stopped.
pub const MAX_OPERATIONS: u64 = 1_000_000;

/// Failure while compiling or running a script.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(String);

impl From<Box<EvalAltResult>> for Error {
    fn from(value: Box<EvalAltResult>) -> Self {
        Self(value.to_string())
    }
}

impl From<rhai::ParseError> for Error {
    fn from(value: rhai::ParseError) -> Self {
        Self(value.to_string())
    }
}

/// Run `source`. Top-level statements run, then `on_load` if that function exists.
pub fn run(world: &mut World, source: &str, filename: &str) -> Result<(), Error> {
    invoke(world, source, filename, None)
}

/// Run `source`, then call the function `name` (no arguments).
pub fn call(world: &mut World, source: &str, filename: &str, name: &str) -> Result<(), Error> {
    invoke(world, source, filename, Some(name))
}

/// `.rhai` files in `dir`, in file-name order. A missing directory is empty.
pub fn plugin_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, Error> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Error(format!("{}: {e}", dir.display())))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("rhai"))
        })
        .collect();
    files.sort();
    Ok(files)
}

fn invoke(
    world: &mut World,
    source: &str,
    filename: &str,
    call: Option<&str>,
) -> Result<(), Error> {
    let shared = Rc::new(RefCell::new(world.clone()));
    let mut engine = Engine::new();
    // No module resolver: `import` cannot read files. The engine has no
    // network functions. Progress stops a runaway loop.
    engine.on_progress(move |ops| {
        if ops > MAX_OPERATIONS {
            Some(Dynamic::from(format!(
                "stopped after {MAX_OPERATIONS} operations"
            )))
        } else {
            None
        }
    });
    bind(&mut engine, shared.clone());
    let ast = engine.compile(source)?;
    let mut scope = Scope::new();
    engine
        .run_ast_with_scope(&mut scope, &ast)
        .map_err(|e| located(filename, e))?;
    if has_fn(&ast, "on_load") {
        call_any(&engine, &mut scope, &ast, filename, "on_load")?;
    }
    for action in shared.borrow().actions() {
        if !has_fn(&ast, &action.id) {
            return Err(Error(format!(
                "{filename}: action \"{}\" has no function of that name",
                action.id
            )));
        }
    }
    if let Some(name) = call {
        if !has_fn(&ast, name) {
            return Err(Error(format!("{filename}: no function \"{name}\"")));
        }
        call_any(&engine, &mut scope, &ast, filename, name)?;
    }
    drop(engine);
    *world = Rc::try_unwrap(shared)
        .map_err(|_| Error("script engine still holds the character".into()))?
        .into_inner();
    Ok(())
}

fn located(filename: &str, err: Box<EvalAltResult>) -> Error {
    let text = match err.as_ref() {
        EvalAltResult::ErrorTerminated(value, pos) => {
            format!("stopped ({value}) at {pos}")
        }
        other => other.to_string(),
    };
    Error(format!("{filename}: {text}"))
}

/// Call a script function and ignore its return value.
///
/// `set` returns a number, so a function that ends on `set(...)` is not `()`.
fn call_any(
    engine: &Engine,
    scope: &mut Scope,
    ast: &AST,
    filename: &str,
    name: &str,
) -> Result<(), Error> {
    let mut args: [Dynamic; 0] = [];
    #[allow(deprecated)]
    engine
        .call_fn_dynamic(scope, ast, false, name, None, &mut args)
        .map(|_| ())
        .map_err(|e| located(filename, e))
}

fn has_fn(ast: &AST, name: &str) -> bool {
    ast.iter_functions()
        .any(|f| f.name == name && f.params.is_empty())
}

fn bind(engine: &mut Engine, shared: Rc<RefCell<World>>) {
    let fail = |e: String| -> Box<EvalAltResult> { e.into() };

    macro_rules! w {
        () => {
            shared.clone()
        };
    }

    {
        let w = w!();
        engine.register_fn("log", move |msg: String| {
            w.borrow_mut().push_log(msg);
        });
    }
    {
        let w = w!();
        engine.register_fn("param_ids", move || w.borrow().param_ids());
    }
    {
        let w = w!();
        engine.register_fn("params", move || -> Array {
            w.borrow()
                .param_ids()
                .into_iter()
                .filter_map(|id| param_map(&w.borrow(), &id).ok())
                .map(Dynamic::from)
                .collect()
        });
    }
    {
        let w = w!();
        engine.register_fn(
            "param",
            move |id: String| -> Result<Map, Box<EvalAltResult>> {
                param_map(&w.borrow(), &id).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "get",
            move |id: String| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
                w.borrow().get(&id).map(|v| v as rhai::FLOAT).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "set",
            move |id: String, value: Dynamic| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
                let n = as_f32(value)?;
                w.borrow_mut()
                    .set(&id, n)
                    .map(|v| v as rhai::FLOAT)
                    .map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "reset",
            move |id: String| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().reset(&id).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn("reset_all", move || w.borrow_mut().reset_all());
    }
    {
        let w = w!();
        engine.register_fn("changed", move || -> Array {
            w.borrow()
                .changed()
                .into_iter()
                .map(Dynamic::from)
                .collect()
        });
    }
    {
        let w = w!();
        engine.register_fn("symmetry", move || w.borrow().symmetry());
    }
    {
        let w = w!();
        engine.register_fn("set_symmetry", move |on: bool| {
            w.borrow_mut().set_symmetry(on)
        });
    }
    {
        let w = w!();
        engine.register_fn("poses", move || -> Array {
            w.borrow()
                .pose_names()
                .into_iter()
                .map(Dynamic::from)
                .collect()
        });
    }
    {
        let w = w!();
        engine.register_fn("pose", move || w.borrow().pose().to_string());
    }
    {
        let w = w!();
        engine.register_fn(
            "set_pose",
            move |name: String| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().set_pose(&name).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn("garments", move || -> Array {
            w.borrow()
                .garment_ids()
                .into_iter()
                .map(Dynamic::from)
                .collect()
        });
    }
    {
        let w = w!();
        engine.register_fn("worn", move || -> Array {
            w.borrow()
                .worn()
                .iter()
                .map(|item| Dynamic::from(item.id.clone()))
                .collect()
        });
    }
    {
        let w = w!();
        engine.register_fn(
            "wear",
            move |id: String| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().wear(&id, None).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "wear",
            move |id: String, tightness: Dynamic| -> Result<(), Box<EvalAltResult>> {
                let t = as_f32(tightness)?;
                w.borrow_mut().wear(&id, Some(t)).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "take_off",
            move |id: String| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().take_off(&id).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "set_tightness",
            move |id: String, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
                let n = as_f32(value)?;
                w.borrow_mut().set_tightness(&id, n).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn(
            "set_garment_color",
            move |id: String,
                  r: Dynamic,
                  g: Dynamic,
                  b: Dynamic|
                  -> Result<(), Box<EvalAltResult>> {
                let rgb = [as_f32(r)?, as_f32(g)?, as_f32(b)?];
                w.borrow_mut().set_garment_color(&id, rgb).map_err(fail)
            },
        );
    }
    {
        let w = w!();
        engine.register_fn("vertex_count", move || {
            w.borrow().vertex_count() as rhai::INT
        });
    }
    {
        let w = w!();
        engine.register_fn("bone_count", move || w.borrow().bone_count() as rhai::INT);
    }
    {
        let w = w!();
        engine.register_fn("slider_count", move || {
            w.borrow().slider_count() as rhai::INT
        });
    }
    {
        let w = w!();
        engine.register_fn(
            "action",
            move |id: String, label: String| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().add_action(&id, &label).map_err(fail)
            },
        );
    }
    bind_amount(engine, shared.clone(), "variation");
    bind_amount(engine, shared.clone(), "detail");
    bind_amount(engine, shared.clone(), "roughness");
    bind_amount(engine, shared.clone(), "freckles");
    bind_amount(engine, shared.clone(), "blush");
    bind_amount(engine, shared.clone(), "eye_shadow");
    bind_amount(engine, shared.clone(), "lips");
    bind_amount(engine, shared.clone(), "iris_angle");
    bind_amount(engine, shared.clone(), "pupil");
    bind_amount(engine, shared.clone(), "eye_size");
    bind_color(engine, shared.clone(), "skin", "skin");
    bind_color(engine, shared.clone(), "freckle_color", "freckle");
    bind_color(engine, shared.clone(), "blush_color", "blush");
    bind_color(engine, shared.clone(), "eye_shadow_color", "eye_shadow");
    bind_color(engine, shared.clone(), "lip_color", "lip");
    bind_color(engine, shared.clone(), "iris", "iris");
    {
        let w = shared.clone();
        engine.register_fn("skin_seed", move || w.borrow().skin_seed() as rhai::INT);
    }
    {
        let w = shared;
        engine.register_fn(
            "set_skin_seed",
            move |n: rhai::INT| -> Result<(), Box<EvalAltResult>> {
                w.borrow_mut().set_skin_seed(n).map_err(|e| e.into())
            },
        );
    }
}

fn bind_amount(engine: &mut Engine, shared: Rc<RefCell<World>>, name: &'static str) {
    let w = shared.clone();
    engine.register_fn(name, move || -> Result<rhai::FLOAT, Box<EvalAltResult>> {
        w.borrow()
            .amount(name)
            .map(|v| v as rhai::FLOAT)
            .map_err(Into::into)
    });
    let w = shared;
    let setter = format!("set_{name}");
    engine.register_fn(
        setter,
        move |value: Dynamic| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let n = as_f32(value)?;
            w.borrow_mut()
                .set_amount(name, n)
                .map(|v| v as rhai::FLOAT)
                .map_err(Into::into)
        },
    );
}

fn bind_color(
    engine: &mut Engine,
    shared: Rc<RefCell<World>>,
    fn_name: &'static str,
    key: &'static str,
) {
    let w = shared.clone();
    engine.register_fn(fn_name, move || -> Result<Array, Box<EvalAltResult>> {
        let rgb = w
            .borrow()
            .color(key)
            .map_err(|e| -> Box<EvalAltResult> { e.into() })?;
        Ok(rgb
            .into_iter()
            .map(|c| Dynamic::from(c as rhai::FLOAT))
            .collect())
    });
    let w = shared;
    let setter = format!("set_{fn_name}");
    engine.register_fn(
        setter,
        move |r: Dynamic, g: Dynamic, b: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let rgb = [as_f32(r)?, as_f32(g)?, as_f32(b)?];
            w.borrow_mut().set_color(key, rgb).map_err(Into::into)
        },
    );
}

fn param_map(world: &World, id: &str) -> Result<Map, String> {
    let def = world.param(id)?;
    let mut map = Map::new();
    map.insert("id".into(), def.id.clone().into());
    map.insert("label".into(), def.label.clone().into());
    let group: Array = def.group.iter().cloned().map(Dynamic::from).collect();
    map.insert("group".into(), group.into());
    map.insert("min".into(), Dynamic::from(def.min as rhai::FLOAT));
    map.insert("max".into(), Dynamic::from(def.max as rhai::FLOAT));
    map.insert("default".into(), Dynamic::from(def.default as rhai::FLOAT));
    map.insert("value".into(), Dynamic::from(world.get(id)? as rhai::FLOAT));
    map.insert(
        "mirror".into(),
        def.mirror.clone().unwrap_or_default().into(),
    );
    Ok(map)
}

fn as_f32(value: Dynamic) -> Result<f32, Box<EvalAltResult>> {
    if let Ok(n) = value.as_float() {
        return Ok(n as f32);
    }
    if let Ok(n) = value.as_int() {
        return Ok(n as f32);
    }
    Err("expected a number".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_morph::{Catalog, ParamDef};

    fn world() -> World {
        let catalog = Catalog {
            params: vec![
                ParamDef {
                    id: "gender".into(),
                    label: "Gender".into(),
                    group: vec!["Body".into(), "Full Body".into()],
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    mirror: None,
                    ends: None,
                },
                ParamDef {
                    id: "nose-width-decr".into(),
                    label: "Nose width decrease".into(),
                    group: vec!["Head".into(), "Nose".into()],
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    mirror: Some("nose-width-incr".into()),
                    ends: None,
                },
                ParamDef {
                    id: "nose-width-incr".into(),
                    label: "Nose width increase".into(),
                    group: vec!["Head".into(), "Nose".into()],
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    mirror: Some("nose-width-decr".into()),
                    ends: None,
                },
            ],
            sliders: vec![],
            layers: vec![],
        };
        World::from_catalog(&catalog, 12, 3)
    }

    #[test]
    fn set_clamps_and_mirrors() {
        let mut w = world();
        run(
            &mut w,
            r#"set("gender", 5); set("nose-width-decr", 0.25);"#,
            "t.rhai",
        )
        .unwrap();
        assert_eq!(w.get("gender").unwrap(), 1.0);
        assert_eq!(w.get("nose-width-decr").unwrap(), 0.25);
        assert_eq!(w.get("nose-width-incr").unwrap(), 0.25);
    }

    #[test]
    fn symmetry_off_writes_one_side() {
        let mut w = world();
        run(
            &mut w,
            r#"set_symmetry(false); set("nose-width-decr", 0.4);"#,
            "t.rhai",
        )
        .unwrap();
        assert_eq!(w.get("nose-width-incr").unwrap(), 0.0);
    }

    #[test]
    fn unknown_parameter_is_an_error() {
        let mut w = world();
        let err = run(&mut w, r#"get("nope");"#, "t.rhai").unwrap_err();
        assert!(err.to_string().contains("unknown parameter"));
    }

    #[test]
    fn pose_and_wardrobe_and_log() {
        let mut w = world();
        run(
            &mut w,
            r#"
                set_pose("wave");
                wear("tee", 0.8);
                wear("hair");
                take_off("hair");
                set_tightness("tee", 0.2);
                set_garment_color("tee", 0.1, 0.2, 0.3);
                log("ready");
            "#,
            "t.rhai",
        )
        .unwrap();
        assert_eq!(w.pose(), "Wave");
        assert_eq!(w.worn().len(), 1);
        assert_eq!(w.worn()[0].id, "tee");
        assert_eq!(w.worn()[0].tightness, Some(0.2));
        assert_eq!(w.worn()[0].color, Some([0.1, 0.2, 0.3]));
        assert_eq!(w.log_lines(), ["ready"]);
    }

    #[test]
    fn action_is_called_by_name() {
        let mut w = world();
        call(
            &mut w,
            r#"
                fn hero() { set("gender", 1); }
                action("hero", "Hero");
            "#,
            "hero.rhai",
            "hero",
        )
        .unwrap();
        assert_eq!(w.get("gender").unwrap(), 1.0);
        assert_eq!(w.actions()[0].label, "Hero");
    }

    #[test]
    fn on_load_runs_after_the_top_level() {
        let mut w = world();
        run(
            &mut w,
            r#"
                let steps = [];
                fn on_load() { log("loaded"); set("gender", 0); }
                set("gender", 1);
            "#,
            "t.rhai",
        )
        .unwrap();
        assert_eq!(w.get("gender").unwrap(), 0.0);
        assert_eq!(w.log_lines(), ["loaded"]);
    }

    #[test]
    fn a_loop_is_stopped() {
        let mut w = world();
        let err = run(&mut w, "loop { }", "t.rhai").unwrap_err();
        assert!(err.to_string().contains("stopped"), "{err}");
    }

    #[test]
    fn integers_are_accepted_as_numbers() {
        let mut w = world();
        run(&mut w, r#"set("gender", 1); set_blush(1);"#, "t.rhai").unwrap();
        assert_eq!(w.get("gender").unwrap(), 1.0);
        assert_eq!(w.material().skin.blush, 1.0);
    }

    #[test]
    fn missing_action_function_fails() {
        let mut w = world();
        let err = run(&mut w, r#"action("hero", "Hero");"#, "t.rhai").unwrap_err();
        assert!(err.to_string().contains("no function"));
    }
}
