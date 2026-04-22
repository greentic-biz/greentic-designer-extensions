#![allow(warnings)]

// Bind against the design-extension world from the component's perspective.
// wasmtime bindgen! generates:
//   - add_to_linker() for `import` items (host implements these for the component)
//   - typed export accessors for `export` items (host calls these on the component)
wasmtime::component::bindgen!({
    path: "wit",
    world: "greentic:extension-design/design-extension",
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
        world: "greentic:extension-deploy/deploy-extension",
    });
}
