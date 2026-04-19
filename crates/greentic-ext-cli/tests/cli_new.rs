use std::process::Command;

fn gtdx_bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target/debug/gtdx");
    p
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("spawn gtdx");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn scaffolds_design_extension_and_lock_file_matches_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let (ok, stdout, stderr) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    for rel in [
        "Cargo.toml",
        "describe.json",
        "src/lib.rs",
        ".gtdx-contract.lock",
        "wit/deps/greentic/extension-base/world.wit",
        "wit/deps/greentic/extension-host/world.wit",
        "wit/deps/greentic/extension-design/world.wit",
    ] {
        assert!(
            proj.join(rel).exists(),
            "missing expected file: {rel}\nstdout:\n{stdout}"
        );
    }

    let lock = std::fs::read_to_string(proj.join(".gtdx-contract.lock")).unwrap();
    assert!(lock.contains("contract_version"));
    assert!(lock.contains("wit/deps/greentic/extension-base/world.wit"));

    // Verify hash in lock matches actual bytes on disk.
    let base_bytes =
        std::fs::read(proj.join("wit/deps/greentic/extension-base/world.wit")).unwrap();
    let expected_sha = {
        use sha2::{Digest, Sha256};
        let d = Sha256::digest(&base_bytes);
        let mut s = String::new();
        for b in d {
            use std::fmt::Write as _;
            write!(&mut s, "{b:02x}").unwrap();
        }
        s
    };
    assert!(
        lock.contains(&format!("sha256:{expected_sha}")),
        "lock file hash did not match on-disk WIT bytes"
    );
}

#[test]
fn scaffolds_bundle_extension_with_correct_wit_deps() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("b");
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("b")
        .arg("--kind")
        .arg("bundle")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new bundle failed: {e}");
    assert!(
        proj.join("wit/deps/greentic/extension-bundle/world.wit")
            .exists()
    );
    assert!(
        !proj
            .join("wit/deps/greentic/extension-design/world.wit")
            .exists()
    );
    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"BundleExtension\""));
}

#[test]
fn scaffolds_deploy_extension_with_correct_wit_deps() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("d");
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("d")
        .arg("--kind")
        .arg("deploy")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new deploy failed: {e}");
    assert!(
        proj.join("wit/deps/greentic/extension-deploy/world.wit")
            .exists()
    );
    assert!(
        !proj
            .join("wit/deps/greentic/extension-bundle/world.wit")
            .exists()
    );
    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"DeployExtension\""));
}

#[test]
fn scaffolds_provider_extension_with_correct_wit_deps() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("p");
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("p")
        .arg("--kind")
        .arg("provider")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new provider failed: {e}");
    assert!(
        proj.join("wit/deps/greentic/extension-provider/world.wit")
            .exists()
    );
    assert!(
        !proj
            .join("wit/deps/greentic/extension-design/world.wit")
            .exists()
    );
    assert!(
        !proj
            .join("wit/deps/greentic/extension-bundle/world.wit")
            .exists()
    );
    assert!(
        !proj
            .join("wit/deps/greentic/extension-deploy/world.wit")
            .exists()
    );

    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"ProviderExtension\""));
    assert!(describe.contains("\"gtpack\""));
    assert!(describe.contains("REPLACE_WITH_YOUR.gtpack"));
}

#[test]
fn target_dir_conflict_without_force_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("something"), "x").unwrap();

    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(!ok);
    assert!(
        e.contains("--force") || e.contains("already exists"),
        "stderr:\n{e}"
    );

    // Pre-existing file must remain untouched.
    let kept = std::fs::read_to_string(proj.join("something")).unwrap();
    assert_eq!(kept, "x");
}

#[test]
fn target_dir_conflict_with_force_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("something"), "x").unwrap();

    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--dir")
        .arg(&proj)
        .arg("--force")
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "stderr:\n{e}");
    assert!(
        !proj.join("something").exists(),
        "old file should be gone after --force"
    );
    assert!(proj.join("Cargo.toml").exists());
}

/// Slow smoke test: generate a project and confirm `cargo check --quiet`
/// succeeds. Gated behind `GTDX_RUN_CARGO_CHECK=1` because it needs network
/// for dep resolution (unless an offline lockfile exists).
#[test]
fn generated_project_passes_cargo_check() {
    if std::env::var("GTDX_RUN_CARGO_CHECK").ok().as_deref() != Some("1") {
        eprintln!("skip: set GTDX_RUN_CARGO_CHECK=1 to run this test");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new failed: {e}");

    let (ok, stdout, stderr) = run(Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(&proj));
    assert!(
        ok,
        "cargo check failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn scaffolded_describe_json_validates_against_schema() {
    let schema_path = {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.push("greentic-ext-contract/schemas/describe-v1.json");
        p
    };
    let schema_bytes = std::fs::read(&schema_path)
        .unwrap_or_else(|e| panic!("read schema at {}: {e}", schema_path.display()));
    let schema: serde_json::Value = serde_json::from_slice(&schema_bytes).unwrap();
    let compiled = jsonschema::validator_for(&schema).expect("compile schema");

    for (kind_flag, scaffold_name) in [
        ("design", "design-demo"),
        ("bundle", "bundle-demo"),
        ("deploy", "deploy-demo"),
        ("provider", "provider-demo"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join(scaffold_name);
        let (ok, stdout, stderr) = run(Command::new(gtdx_bin())
            .arg("new")
            .arg(scaffold_name)
            .arg("--kind")
            .arg(kind_flag)
            .arg("--dir")
            .arg(&proj)
            .arg("-y")
            .arg("--no-git"));
        assert!(
            ok,
            "gtdx new --kind {kind_flag} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );

        let describe_bytes = std::fs::read(proj.join("describe.json")).unwrap();
        let describe: serde_json::Value = serde_json::from_slice(&describe_bytes).unwrap();
        if !compiled.is_valid(&describe) {
            let details: Vec<String> = compiled
                .iter_errors(&describe)
                .map(|e| format!("- {e}"))
                .collect();
            panic!(
                "describe.json for kind={kind_flag} failed schema validation:\n{}",
                details.join("\n")
            );
        }
    }
}
