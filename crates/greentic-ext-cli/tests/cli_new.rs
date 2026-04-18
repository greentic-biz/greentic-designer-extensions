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
    assert!(proj.join("wit/deps/greentic/extension-bundle/world.wit").exists());
    assert!(!proj.join("wit/deps/greentic/extension-design/world.wit").exists());
    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"bundle\""));
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
    assert!(proj.join("wit/deps/greentic/extension-deploy/world.wit").exists());
    assert!(!proj.join("wit/deps/greentic/extension-bundle/world.wit").exists());
    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"deploy\""));
}
