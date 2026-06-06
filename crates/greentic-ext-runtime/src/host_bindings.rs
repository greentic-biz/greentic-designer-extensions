#![allow(warnings)]

// Bind against the design-extension world from the component's perspective.
// wasmtime bindgen! generates:
//   - add_to_linker() for `import` items (host implements these for the component)
//   - typed export accessors for `export` items (host calls these on the component)
wasmtime::component::bindgen!({
    path: "wit",
    world: "greentic:extension-design/design-extension@0.2.0",
});

// ---------------------------------------------------------------------------
// Deploy-extension bindings
//
// Generated in a sibling `mod deploy` to keep the type namespace isolated
// from the design-extension bindings above. Both worlds share
// `greentic:extension-base` + `greentic:extension-host/*`, so generating
// them in the root module would cause duplicate-type errors.
// ---------------------------------------------------------------------------
pub mod deploy {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "greentic:extension-deploy/deploy-extension@0.1.0",
    });
}

// ---------------------------------------------------------------------------
// Bundle-extension bindings
//
// Same pattern as deploy: isolated submodule because all three worlds share
// `extension-base` + `extension-host` types and re-binding them at the root
// would conflict. The bundle world exports `recipes` and `bundling` (the
// host calls `bundling.render(...)` in `runtime::render_bundle`); it imports
// the standard host triplet (logging / i18n / broker), wired through the
// shared `add_to_linker` helpers.
// ---------------------------------------------------------------------------
pub mod bundle {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "greentic:extension-bundle/bundle-extension@0.1.0",
    });
}

// ---------------------------------------------------------------------------
// DW-Composer-extension bindings
//
// Isolated submodule for the `greentic:dw-composer/dw-composer-extension`
// world. Composer extensions export a domain-specific `composer` interface
// (`greentic:dw-composer/composer@0.1.0`) instead of the generic
// `greentic:extension-design/tools` interface. Keeping them in their own
// submodule avoids duplicate-type conflicts with the design-extension
// bindings above.
// ---------------------------------------------------------------------------
pub mod dw_composer {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "greentic:dw-composer/dw-composer-extension@0.1.0",
    });
}
