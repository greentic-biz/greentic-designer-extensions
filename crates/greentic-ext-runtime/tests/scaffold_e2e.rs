//! End-to-end smoke: scaffold a design extension via `gtdx new`, build the
//! release WASM + pack with `gtdx dev --once --no-install`, unpack the produced
//! `.gtxpack` into a runtime discovery directory, load it via
//! `greentic-ext-runtime`, and invoke the scaffolded `tools::invoke_tool`.
//!
//! The scaffold's TODO-stub returns `ExtensionError::InvalidInput("unknown
//! tool: <name>")` so a successful loop ends with the runtime surfacing that
//! error back through the wasmtime dispatch — proving the full pipeline
//! (scaffold → cargo-component build → pack → discovery → component
//! instantiation → typed-function call → host-side error decoding) works.
//!
//! Gated behind `GTDX_RUN_BUILD=1` because it requires cargo-component on
//! PATH. Skips silently otherwise.

use std::path::PathBuf;
use std::process::Command;

use ed25519_dalek::SigningKey;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use rand::rngs::OsRng;

fn gtdx_bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target/debug/gtdx");
    p
}

fn gate() -> bool {
    std::env::var("GTDX_RUN_BUILD").ok().as_deref() == Some("1")
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("spawn");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn scaffolded_design_extension_loads_and_invoke_tool_returns_stub_error() {
    if !gate() {
        eprintln!("skipped: set GTDX_RUN_BUILD=1 to enable (requires cargo-component)");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let home = tmp.path().join("home");

    // 1. Scaffold.
    let (ok, o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--id")
        .arg("com.example.demo")
        .arg("--dir")
        .arg(&proj)
        .arg("--author")
        .arg("tester")
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new failed\nstdout:\n{o}\nstderr:\n{e}");

    // 2. Build + pack via gtdx dev --once --no-install.
    let (ok, o, e) = run(Command::new(gtdx_bin())
        .env("GREENTIC_HOME", &home)
        .arg("dev")
        .arg("--once")
        .arg("--no-install")
        .arg("--manifest")
        .arg(proj.join("Cargo.toml")));
    assert!(ok, "gtdx dev --once failed\nstdout:\n{o}\nstderr:\n{e}");

    // 3. Locate the produced .gtxpack.
    let dist = proj.join("dist");
    let pack = std::fs::read_dir(&dist)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("gtxpack"))
        .expect("no .gtxpack landed in dist/");

    // 4. Unpack into a runtime discovery layout: <user_root>/<kind>/<id>-<version>/.
    let user_root = tmp.path().join("user");
    std::fs::create_dir_all(user_root.join("design")).unwrap();
    let ext_dir = user_root.join("design/com.example.demo-0.1.0");
    greentic_ext_testing::unpack_to_dir(&pack, &ext_dir).unwrap();

    // 5. Sign describe.json so the runtime's signature gate accepts the load.
    let describe_path = ext_dir.join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_ext_contract::DescribeJson = serde_json::from_str(&raw).unwrap();
    let sk = SigningKey::generate(&mut OsRng);
    greentic_ext_contract::sign_describe(&mut describe, &sk).expect("sign describe");
    std::fs::write(
        &describe_path,
        serde_json::to_string_pretty(&describe).unwrap(),
    )
    .unwrap();

    // 6. Register the extension directly (we don't rely on the watcher here —
    //    we want a deterministic load followed by a synchronous invoke).
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root.clone()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&ext_dir)
        .expect("register scaffolded ext");

    // 7. Invoke a tool — the scaffold implements `tools::invoke_tool` as a
    //    TODO-stub that always returns ExtensionError::InvalidInput. The
    //    runtime wraps the guest error in RuntimeError::Wasmtime with the
    //    original message preserved in its Display form.
    let result = rt.invoke_tool("com.example.demo", "something", "{}");
    match result {
        Ok(s) => panic!(
            "expected the scaffold's TODO stub to error, got Ok: {s}. \
             If the scaffold template changed, update this assertion accordingly."
        ),
        Err(err) => {
            let msg = format!("{err}");
            assert!(
                msg.contains("unknown tool") && msg.contains("something"),
                "expected the wrapped error to mention 'unknown tool' and 'something'; got: {msg}"
            );
        }
    }
}
