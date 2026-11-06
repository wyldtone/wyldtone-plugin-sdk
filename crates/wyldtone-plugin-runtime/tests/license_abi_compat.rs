//! Compatibility fixtures for the proposed optional license-delivery WASM ABI.
//! No DRM, JWT validation or production host implementation is included here.
//! This checks the wire-level calling convention before it is frozen.

use wasmtime::{Engine, Instance, Module, Store};

const HEADER: &str = include_str!("../../../abi/include/wasm_plugin.h");

// Legacy plugin: no license exports and, crucially, no new imports required.
const LEGACY: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "plugin_init") (param i32 i32 i32))
)
"#;

// Test fixture models an opt-in plugin that requests an opaque grant and
// consumes the host-provided bytes from its own linear memory.
const LICENSE_AWARE: &str = r#"
(module
  (memory (export "memory") 1)
  (global $requested (mut i32) (i32.const 1))
  (global $last_size (mut i32) (i32.const -1))
  (func (export "plugin_license_abi_version") (result i32)
    i32.const 1)
  (func (export "plugin_license_requested") (result i32)
    global.get $requested)
  (func (export "plugin_license_prepare_load") (param $size i32) (result i32)
    local.get $size
    i32.const 65536
    i32.gt_u
    if (result i32)
      i32.const -1
    else
      i32.const 1024
    end)
  (func (export "plugin_license_commit_load") (param $size i32) (result i32)
    local.get $size
    global.set $last_size
    i32.const 0
    global.set $requested
    i32.const 0)
  (func (export "fixture_last_size") (result i32)
    global.get $last_size)
)
"#;

#[test]
fn the_public_header_declares_the_stable_optional_contract() {
    for signature in [
        "int32_t plugin_license_abi_version(void);",
        "int32_t plugin_license_requested(void);",
        "int32_t plugin_license_prepare_load(int32_t size);",
        "int32_t plugin_license_commit_load(int32_t size);",
        "#define WASM_PLUGIN_LICENSE_ABI_VERSION 1",
        "#define WASM_PLUGIN_MAX_LICENSE_BYTES 65536",
        "#define WASM_PLUGIN_EVENT_LICENSE_REQUESTED 0x04",
    ] {
        assert!(HEADER.contains(signature), "Missing ABI declaration: {signature}");
    }
}

#[test]
fn older_wasm_modules_still_instantiate_without_license_imports() {
    let engine = Engine::default();
    let module = Module::new(&engine, LEGACY).unwrap();
    assert!(module.imports().next().is_none());
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    for name in [
        "plugin_license_abi_version",
        "plugin_license_requested",
        "plugin_license_prepare_load",
        "plugin_license_commit_load",
    ] {
        assert!(instance.get_func(&mut store, name).is_none());
    }
    assert!(instance.get_typed_func::<(i32, i32, i32), ()>(&mut store, "plugin_init").is_ok());
}

#[test]
fn licensed_plugin_requests_and_receives_opaque_bytes() {
    let engine = Engine::default();
    let module = Module::new(&engine, LICENSE_AWARE).unwrap();
    assert!(module.imports().next().is_none());
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let version = instance.get_typed_func::<(), i32>(&mut store, "plugin_license_abi_version").unwrap();
    let requested = instance.get_typed_func::<(), i32>(&mut store, "plugin_license_requested").unwrap();
    let prepare = instance.get_typed_func::<i32, i32>(&mut store, "plugin_license_prepare_load").unwrap();
    let commit = instance.get_typed_func::<i32, i32>(&mut store, "plugin_license_commit_load").unwrap();
    let last_size = instance.get_typed_func::<(), i32>(&mut store, "fixture_last_size").unwrap();

    assert_eq!(version.call(&mut store, ()).unwrap(), 1);
    assert_eq!(requested.call(&mut store, ()).unwrap(), 1);

    // The future host must validate the pointer and copy exactly these bytes.
    let grant = b"opaque.signed.license";
    let ptr = prepare.call(&mut store, grant.len() as i32).unwrap();
    assert!(ptr > 0);
    let memory = instance.get_memory(&mut store, "memory").unwrap();
    memory.write(&mut store, ptr as usize, grant).unwrap();
    assert_eq!(memory.data(&store)[ptr as usize..ptr as usize + grant.len()], grant[..]);
    assert_eq!(commit.call(&mut store, grant.len() as i32).unwrap(), 0);
    assert_eq!(last_size.call(&mut store, ()).unwrap(), grant.len() as i32);
    assert_eq!(requested.call(&mut store, ()).unwrap(), 0);
    assert_eq!(prepare.call(&mut store, 65537).unwrap(), -1);
}

#[test]
fn absence_of_license_can_be_reported_as_zero_length() {
    let engine = Engine::default();
    let module = Module::new(&engine, LICENSE_AWARE).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let prepare = instance.get_typed_func::<i32, i32>(&mut store, "plugin_license_prepare_load").unwrap();
    let commit = instance.get_typed_func::<i32, i32>(&mut store, "plugin_license_commit_load").unwrap();
    assert!(prepare.call(&mut store, 0).unwrap() >= 0);
    assert_eq!(commit.call(&mut store, 0).unwrap(), 0);
}
