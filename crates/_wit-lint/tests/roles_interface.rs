use std::fs;

#[test]
fn extension_design_declares_roles_interface_at_0_2_0() {
    let wit = fs::read_to_string("../../wit/extension-design.wit")
        .expect("read extension-design.wit");
    assert!(
        wit.contains("package greentic:extension-design@0.2.0;"),
        "package version must be bumped to 0.2.0 (found header: {:?})",
        wit.lines().next()
    );
    assert!(
        wit.contains("interface roles {"),
        "expected `interface roles {{` block"
    );
    assert!(
        wit.contains("list-roles: func() -> list<role-spec>;"),
        "expected list-roles signature"
    );
    assert!(
        wit.contains("compile-role: func("),
        "expected compile-role signature"
    );
    assert!(
        wit.contains("validate-role: func("),
        "expected validate-role signature"
    );
    assert!(
        wit.contains("export roles;"),
        "world design-extension must export roles"
    );
}
