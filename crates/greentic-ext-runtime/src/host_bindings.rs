#![allow(warnings)]

// Bind against the design-extension world from the component's perspective.
// wasmtime bindgen! generates:
//   - add_to_linker() for `import` items (host implements these for the component)
//   - typed export accessors for `export` items (host calls these on the component)
wasmtime::component::bindgen!({
    path: "wit",
    world: "greentic:extension-design/design-extension",
});
