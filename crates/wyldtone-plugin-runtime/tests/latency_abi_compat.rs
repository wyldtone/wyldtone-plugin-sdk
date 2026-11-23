//! Optional processor-latency ABI wire compatibility. No new imports.
use wasmtime::{Engine, Instance, Module, Store};

const HEADER: &str = include_str!("../../../abi/include/wasm_plugin.h");
const OLD_WAP: &str = r#"(module (memory (export "memory") 1)
  (func (export "plugin_init") (param i32 i32 i32)))"#;
const VARIABLE_LATENCY_WAP: &str = r#"(module
 (memory (export "memory") 1)
 (global $mode (mut i32) (i32.const 0))
 (func (export "plugin_init") (param i32 i32 i32)
   i32.const 0 global.set $mode)
 (func (export "plugin_parameter_set") (param i32 f32)
   local.get 1 f32.const 0.75 f32.gt
   if i32.const 46 global.set $mode
   else
     local.get 1 f32.const 0.25 f32.gt
     if i32.const 31 global.set $mode
     else i32.const 0 global.set $mode end
   end)
 (func (export "plugin_latency_samples") (result i32)
   global.get $mode)
 (func (export "plugin_max_latency_samples") (result i32)
   i32.const 46)
)"#;

#[test]
fn header_defines_optional_current_and_max_contract() {
    for symbol in [
        "int32_t plugin_latency_samples(void);",
        "int32_t plugin_max_latency_samples(void);",
        "#define WASM_PLUGIN_EVENT_LATENCY_CHANGED 0x08",
    ] {
        assert!(HEADER.contains(symbol), "missing ABI symbol: {symbol}");
    }
}
#[test]
fn older_waps_have_no_additional_required_imports() {
    let engine = Engine::default();
    let module = Module::new(&engine, OLD_WAP).unwrap();
    assert!(module.imports().next().is_none());
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    assert!(instance.get_func(&mut store, "plugin_latency_samples").is_none());
    assert!(instance.get_func(&mut store, "plugin_max_latency_samples").is_none());
}
#[test]
fn reports_correct_latency_after_host_parameter_update() {
    let engine = Engine::default();
    let module = Module::new(&engine, VARIABLE_LATENCY_WAP).unwrap();
    assert!(module.imports().next().is_none());
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let init = instance.get_typed_func::<(i32,i32,i32),()>(&mut store, "plugin_init").unwrap();
    let set = instance.get_typed_func::<(i32,f32),()>(&mut store, "plugin_parameter_set").unwrap();
    let current = instance.get_typed_func::<(),i32>(&mut store, "plugin_latency_samples").unwrap();
    let max = instance.get_typed_func::<(),i32>(&mut store, "plugin_max_latency_samples").unwrap();
    init.call(&mut store, (48000,2,256)).unwrap();
    assert_eq!(max.call(&mut store, ()).unwrap(), 46);
    for (normalized, expected) in [(0.0,0), (0.5,31), (1.0,46), (0.0,0)] {
        set.call(&mut store, (12,normalized)).unwrap();
        assert_eq!(current.call(&mut store, ()).unwrap(), expected);
    }
}
