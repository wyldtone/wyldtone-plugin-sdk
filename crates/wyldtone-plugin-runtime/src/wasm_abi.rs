use std::collections::HashSet;
use std::path::Path;

use wasmtime::{
    Cache,
    CacheConfig,
    Engine,
    Linker,
    Memory,
    Module,
    Store,
    TypedFunc,
};

use wasmtime_wasi::{
    p1::WasiP1Ctx,
    WasiCtxBuilder,
};

use wyldtone_plugin_api::parameter::{
    Parameter,
    ParameterKind,
    ParameterValue,
};

/// Parameter type flags used by the WASM plugin ABI.
///
/// bit 0 = automatable
/// bit 1 = Int
/// bit 2 = Bool
/// bit 3 = Choice
///
/// If none of the type bits are set, the parameter is treated as Float.
pub const FLAG_AUTOMATABLE: i32 = 1;
pub const FLAG_INT: i32 = 2;
pub const FLAG_BOOL: i32 = 4;
pub const FLAG_CHOICE: i32 = 8;

/// State associated with one WASM instance.
///
/// WASI is deliberately kept separate from the plugin itself so that
/// capabilities can later be configured per plugin instance.
pub struct WasmStoreState {
    pub wasi: WasiP1Ctx,
}

/// Shared WASM plugin runtime and ABI.
///
/// This contains everything required to instantiate a WASM plugin,
/// communicate with its parameter ABI and access its linear memory.
///
/// Audio processing is also exposed here so that both the audio engine
/// and other tools can use the exact same WASM implementation.
pub struct WasmPlugin {
    pub store: Store<WasmStoreState>,
    pub memory: Memory,

    pub plugin_create: Option<TypedFunc<(), ()>>,
    pub plugin_init: TypedFunc<(i32, i32, i32), ()>,
    pub plugin_reset: Option<TypedFunc<(), ()>>,
    pub plugin_process: TypedFunc<(i32, i32), ()>,
    pub plugin_process_buses: Option<TypedFunc<(i32, i32), ()>>,
    pub plugin_buffer_ptr: TypedFunc<(), i32>,

    pub plugin_name: TypedFunc<(i32, i32), i32>,

    pub plugin_parameter_count: TypedFunc<(), i32>,
    pub plugin_parameter_get: TypedFunc<(i32,), f32>,
    pub plugin_parameter_set: TypedFunc<(i32, f32), ()>,

    pub plugin_pending_events:
       Option<TypedFunc<(), i32>>,

    // Optional bus-layout metadata ABI. Older plugins that do not export
    // these functions fall back to the host's legacy mono layout.
    pub plugin_input_bus_count:
        Option<TypedFunc<(), i32>>,
    pub plugin_output_bus_count:
        Option<TypedFunc<(), i32>>,
    pub plugin_input_bus_channels:
        Option<TypedFunc<(i32,), i32>>,
    pub plugin_output_bus_channels:
        Option<TypedFunc<(i32,), i32>>,

    pub plugin_state_version:
        Option<TypedFunc<(), i32>>,
    pub plugin_state_capture:
        Option<TypedFunc<(), i32>>,
    pub plugin_state_buffer_ptr:
        Option<TypedFunc<(), i32>>,
    pub plugin_state_prepare_load:
        Option<TypedFunc<(i32,), i32>>,
    pub plugin_state_commit_load:
        Option<TypedFunc<(i32,), ()>>,

    pub buffer_ptr: i32,

    pub plugin_parameter_id:
        TypedFunc<(i32, i32, i32), i32>,
    pub plugin_parameter_name:
        TypedFunc<(i32, i32, i32), i32>,
    pub plugin_parameter_label:
        TypedFunc<(i32, i32, i32), i32>,

    pub plugin_parameter_min:
        TypedFunc<(i32,), f32>,
    pub plugin_parameter_max:
        TypedFunc<(i32,), f32>,
    pub plugin_parameter_default:
        TypedFunc<(i32,), f32>,
    pub plugin_parameter_step:
        TypedFunc<(i32,), f32>,

    pub plugin_parameter_flags:
        TypedFunc<(i32,), i32>,

    pub plugin_parameter_choice:
        Option<TypedFunc<(i32, i32, i32, i32), i32>>,
}

impl WasmPlugin {
    /// Create a Wasmtime engine configured for the plugin runtime.
    pub fn create_engine()
        -> Result<Engine, Box<dyn std::error::Error>>
    {
        let mut config = wasmtime::Config::new();

        // WASI is enabled at the linker/context level.
        //
        // The WASI context currently receives no arguments,
        // environment variables, preopened directories or other
        // explicit host resources.
        //
        // Capabilities can be added here later on a per-plugin basis.
        config.wasm_multi_memory(false);

        // Activate Backtrace Debug
        config.wasm_backtrace_details(
            wasmtime::WasmBacktraceDetails::Enable,
        );

        // Keep Wasmtime's compiled-module cache inside the current project
        // for now. Wasmtime requires an absolute cache path.
        //
        // The cache is persistent across application restarts and allows
        // Module::new() to reuse compiled native code when the WASM bytes
        // and engine configuration have not changed.
        let cache_dir = std::env::current_dir()?
            .join("wasm-cache");

        std::fs::create_dir_all(&cache_dir)?;

        let mut cache_config = CacheConfig::new();
        cache_config.with_directory(cache_dir);

        let cache = Cache::new(cache_config)?;
        config.cache(Some(cache));

        Ok(Engine::new(&config)?)
    }

    /// Load a WASM plugin from a file.
    ///
    /// Plugin initialization is deliberately deferred until
    /// `prepare()` is called.
    pub fn from_file(
        engine: &Engine,
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let wasm = std::fs::read(path)?;

        Self::from_bytes(
            engine,
            &wasm,
        )
    }

    /// Load a WASM plugin from bytes in memory.
    ///
    /// Plugin initialization is deliberately deferred until
    /// `prepare()` is called.
    pub fn from_bytes(
        engine: &Engine,
        wasm: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let module =
            Module::new(engine, wasm)?;

        Self::from_module(
            engine,
            &module,
        )
    }

    /// Instantiate a compiled WASM module.
    ///
    /// Plugin initialization is deliberately not performed here.
    /// Audio-specific initialization is handled by `prepare()`.
    pub fn from_module(
        engine: &Engine,
        module: &Module,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut linker =
            Linker::<WasmStoreState>::new(engine);

        wasmtime_wasi::p1::add_to_linker_sync(
            &mut linker,
            |state| &mut state.wasi,
        )?;

        let wasi =
            WasiCtxBuilder::new()
                .build_p1();

        let mut store =
            Store::new(
                engine,
                WasmStoreState { wasi },
            );

        let instance =
            linker.instantiate(
                &mut store,
                module,
            )?;

        // Only emit instantiation diagnostics when explicitly requested.
        // The DSP probe creates many fresh instances, so unconditional logging
        // would otherwise flood normal CLI output.
        let debug_logging = std::env::var("WYLDTONE_SDK_DEBUG")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // Initialize WASI reactor if the module exports _initialize.
        if let Some(initialize) = instance.get_func(&mut store, "_initialize") {
            let initialize = initialize.typed::<(), ()>(&store)?;
            initialize.call(&mut store, ())?;

            if debug_logging {
                eprintln!("[DEBUG] WASM runtime initialized.");
            }
        } else if debug_logging {
            eprintln!("[DEBUG] No _initialize export found. Skipping initialization.");
        }

        let memory =
            instance
                .get_memory(
                    &mut store,
                    "memory",
                )
                .ok_or(
                    "WASM plugin does not export `memory`",
                )?;

        let plugin_create =
            instance
                .get_typed_func::<
                    (),
                    (),
                >(
                    &mut store,
                    "plugin_create",
                )
                .ok();
        
        let plugin_init =
            instance
                .get_typed_func::<
                    (i32, i32, i32),
                    (),
                >(
                    &mut store,
                    "plugin_init",
                )?;

        let plugin_reset = 
            instance
                .get_typed_func::<(), ()>(
                    &mut store,
                    "plugin_reset",
                )
                .ok();

        let plugin_pending_events =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_pending_events",
                )
                .ok();

        let plugin_input_bus_count =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_input_bus_count",
                )
                .ok();

        let plugin_output_bus_count =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_output_bus_count",
                )
                .ok();

        let plugin_input_bus_channels =
            instance
                .get_typed_func::<(i32,), i32>(
                    &mut store,
                    "plugin_input_bus_channels",
                )
                .ok();

        let plugin_output_bus_channels =
            instance
                .get_typed_func::<(i32,), i32>(
                    &mut store,
                    "plugin_output_bus_channels",
                )
                .ok();

        let plugin_state_version =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_state_version",
                )
                .ok();

        let plugin_state_capture =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_state_capture",
                )
                .ok();

        let plugin_state_buffer_ptr =
            instance
                .get_typed_func::<(), i32>(
                    &mut store,
                    "plugin_state_buffer_ptr",
                )
                .ok();

        let plugin_state_prepare_load =
            instance
                .get_typed_func::<(i32,), i32>(
                    &mut store,
                    "plugin_state_prepare_load",
                )
                .ok();

        let plugin_state_commit_load =
            instance
                .get_typed_func::<(i32,), ()>(
                    &mut store,
                    "plugin_state_commit_load",
                )
                .ok();

        let plugin_process =
            instance
                .get_typed_func::<
                    (i32, i32),
                    (),
                >(
                    &mut store,
                    "plugin_process",
                )?;

        let plugin_process_buses = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "plugin_process_buses")
            .ok();

        let plugin_buffer_ptr =
            instance
                .get_typed_func::<
                    (),
                    i32,
                >(
                    &mut store,
                    "plugin_buffer_ptr",
                )?;

        let plugin_name =
            instance
                .get_typed_func::<
                    (i32, i32),
                    i32,
                >(
                    &mut store,
                    "plugin_name",
                )?;

        let plugin_parameter_count =
            instance
                .get_typed_func::<
                    (),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_count",
                )?;

        let plugin_parameter_get =
            instance
                .get_typed_func::<
                    (i32,),
                    f32,
                >(
                    &mut store,
                    "plugin_parameter_get",
                )?;

        let plugin_parameter_set =
            instance
                .get_typed_func::<
                    (i32, f32),
                    (),
                >(
                    &mut store,
                    "plugin_parameter_set",
                )?;

        let plugin_parameter_id =
            instance
                .get_typed_func::<
                    (i32, i32, i32),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_id",
                )?;

        let plugin_parameter_name =
            instance
                .get_typed_func::<
                    (i32, i32, i32),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_name",
                )?;

        let plugin_parameter_label =
            instance
                .get_typed_func::<
                    (i32, i32, i32),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_label",
                )?;

        let plugin_parameter_min =
            instance
                .get_typed_func::<
                    (i32,),
                    f32,
                >(
                    &mut store,
                    "plugin_parameter_min",
                )?;

        let plugin_parameter_max =
            instance
                .get_typed_func::<
                    (i32,),
                    f32,
                >(
                    &mut store,
                    "plugin_parameter_max",
                )?;

        let plugin_parameter_default =
            instance
                .get_typed_func::<
                    (i32,),
                    f32,
                >(
                    &mut store,
                    "plugin_parameter_default",
                )?;

        let plugin_parameter_step =
            instance
                .get_typed_func::<
                    (i32,),
                    f32,
                >(
                    &mut store,
                    "plugin_parameter_step",
                )?;

        let plugin_parameter_flags =
            instance
                .get_typed_func::<
                    (i32,),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_flags",
                )?;

        let plugin_parameter_choice =
            instance
                .get_typed_func::<
                    (i32, i32, i32, i32),
                    i32,
                >(
                    &mut store,
                    "plugin_parameter_choice",
                )
                .ok();
        
        let buffer_ptr =
            plugin_buffer_ptr.call(
                &mut store,
                (),
            )?;

        if let Some(plugin_create) = &plugin_create {
            plugin_create.call(
                &mut store,
                (),
            )?;
        }

        Ok(Self {
            store,
            memory,

            plugin_create,
            plugin_init,
            plugin_reset,
            plugin_process,
            plugin_process_buses,
            plugin_buffer_ptr,

            plugin_name,

            plugin_parameter_count,
            plugin_parameter_get,
            plugin_parameter_set,

            plugin_pending_events,

            plugin_input_bus_count,
            plugin_output_bus_count,
            plugin_input_bus_channels,
            plugin_output_bus_channels,

            plugin_state_version,
            plugin_state_capture,
            plugin_state_buffer_ptr,
            plugin_state_prepare_load,
            plugin_state_commit_load,

            buffer_ptr,

            plugin_parameter_id,
            plugin_parameter_name,
            plugin_parameter_label,

            plugin_parameter_min,
            plugin_parameter_max,
            plugin_parameter_default,
            plugin_parameter_step,

            plugin_parameter_flags,
            plugin_parameter_choice,
        })
    }

    /// Read the plugin's declared audio bus layout.
    ///
    /// The bus-layout ABI was added after the original plugin ABI, so all
    /// four exports are optional for backwards compatibility. If none of
    /// them are present, `None` is returned and the caller may apply a
    /// legacy/default layout. A partially implemented bus-layout ABI is
    /// treated as an error.
    pub fn bus_layout_channels(
        &mut self,
    ) -> Result<
        Option<(Vec<usize>, Vec<usize>)>,
        Box<dyn std::error::Error>,
    > {
        let exports_present = [
            self.plugin_input_bus_count.is_some(),
            self.plugin_output_bus_count.is_some(),
            self.plugin_input_bus_channels.is_some(),
            self.plugin_output_bus_channels.is_some(),
        ];

        if exports_present.iter().all(|present| !present) {
            return Ok(None);
        }

        if !exports_present.iter().all(|present| *present) {
            return Err(
                "WASM plugin exports an incomplete bus-layout ABI".into(),
            );
        }

        let input_count = self
            .plugin_input_bus_count
            .as_ref()
            .expect("bus ABI completeness checked")
            .call(&mut self.store, ())?;

        let output_count = self
            .plugin_output_bus_count
            .as_ref()
            .expect("bus ABI completeness checked")
            .call(&mut self.store, ())?;

        if input_count < 0 || output_count < 0 {
            return Err("WASM plugin returned a negative bus count".into());
        }

        // Keep obviously corrupt/malicious metadata from causing huge host
        // allocations. This is deliberately generous for future sidechains.
        const MAX_BUSES: i32 = 32;
        const MAX_CHANNELS_PER_BUS: i32 = 64;

        if input_count > MAX_BUSES || output_count > MAX_BUSES {
            return Err("WASM plugin returned too many audio buses".into());
        }

        let mut input_buses = Vec::with_capacity(input_count as usize);
        let mut output_buses = Vec::with_capacity(output_count as usize);

        let input_channels_fn = self
            .plugin_input_bus_channels
            .as_ref()
            .expect("bus ABI completeness checked");

        for bus_index in 0..input_count {
            let channels = input_channels_fn
                .call(&mut self.store, (bus_index,))?;

            if !(0..=MAX_CHANNELS_PER_BUS).contains(&channels) {
                return Err(
                    format!(
                        "WASM plugin returned invalid input channel count {} for bus {}",
                        channels, bus_index
                    )
                    .into(),
                );
            }

            input_buses.push(channels as usize);
        }

        let output_channels_fn = self
            .plugin_output_bus_channels
            .as_ref()
            .expect("bus ABI completeness checked");

        for bus_index in 0..output_count {
            let channels = output_channels_fn
                .call(&mut self.store, (bus_index,))?;

            if !(0..=MAX_CHANNELS_PER_BUS).contains(&channels) {
                return Err(
                    format!(
                        "WASM plugin returned invalid output channel count {} for bus {}",
                        channels, bus_index
                    )
                    .into(),
                );
            }

            output_buses.push(channels as usize);
        }

        Ok(Some((input_buses, output_buses)))
    }

    /// Prepare the WASM plugin for audio processing.
    ///
    /// This is deliberately separate from construction.
    ///
    /// The host supplies the runtime-specific audio context here,
    /// allowing the same WASM module to be instantiated first and
    /// prepared later with the actual sample rate and channel count.
    pub fn prepare(
        &mut self,
        sample_rate: u32,
        channels: usize,
        block_size: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.plugin_init.call(
            &mut self.store,
            (
                sample_rate as i32,
                channels as i32,
                block_size as i32,
            ),
        )?;

        self.buffer_ptr =
            self.plugin_buffer_ptr.call(
                &mut self.store,
                (),
            )?;

        Ok(())
    }

    pub fn reset(&mut self) {
        if let Some(plugin_reset) = &self.plugin_reset {
            plugin_reset
                .call(&mut self.store, ())
                .expect("WASM plugin reset failed");
        }
    }

    pub fn take_pending_events(&mut self) -> u8 {
        let Some(plugin_pending_events) =
            &self.plugin_pending_events
        else {
            return 0;
        };

        plugin_pending_events
            .call(&mut self.store, ())
            .unwrap_or(0) as u8
    }

    pub fn state_version(&mut self) -> u32 {
        let Some(plugin_state_version) =
            &self.plugin_state_version
        else {
            return 1;
        };

        plugin_state_version
            .call(&mut self.store, ())
            .unwrap_or(1)
            .max(1) as u32
    }

    pub fn save_state(
        &mut self,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (Some(capture), Some(buffer_ptr)) = (
            &self.plugin_state_capture,
            &self.plugin_state_buffer_ptr,
        ) else {
            return Ok(Vec::new());
        };

        let size = capture.call(&mut self.store, ())?;

        if size <= 0 {
            return Ok(Vec::new());
        }

        let ptr = buffer_ptr.call(&mut self.store, ())?;

        if ptr <= 0 {
            return Err("plugin returned an invalid state buffer pointer".into());
        }

        let start = ptr as usize;
        let end = start
            .checked_add(size as usize)
            .ok_or("plugin state buffer range overflow")?;

        let memory = self.memory.data(&self.store);

        if end > memory.len() {
            return Err("plugin state buffer exceeds WASM memory".into());
        }

        Ok(memory[start..end].to_vec())
    }

    pub fn load_state(
        &mut self,
        state: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (Some(prepare), Some(commit)) = (
            &self.plugin_state_prepare_load,
            &self.plugin_state_commit_load,
        ) else {
            return Ok(());
        };

        let size = i32::try_from(state.len())
            .map_err(|_| "plugin state is too large")?;

        let ptr = prepare.call(&mut self.store, (size,))?;

        if !state.is_empty() {
            if ptr <= 0 {
                return Err("plugin returned an invalid state load buffer pointer".into());
            }

            self.memory.write(
                &mut self.store,
                ptr as usize,
                state,
            )?;
        }

        commit.call(&mut self.store, (size,))?;

        Ok(())
    }

    fn ensure_buffer_capacity(
        &mut self,
        byte_len: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current =
            self.memory.data_size(
                &self.store,
            );

        if current < byte_len {
            let missing =
                byte_len - current;

            let pages =
                (missing + 65_535) / 65_536;

            self.memory.grow(
                &mut self.store,
                pages as u64,
            )?;
        }

        Ok(())
    }

    fn read_plugin_string(
        &mut self,
        func: TypedFunc<
            (i32, i32, i32),
            i32,
        >,
        index: i32,
    ) -> Result<
        String,
        Box<dyn std::error::Error>,
    > {
        const BUFFER_SIZE: usize = 256;

        let buffer_ptr =
            self.buffer_ptr;

        let required =
            buffer_ptr
                .checked_add(
                    BUFFER_SIZE as i32,
                )
                .ok_or(
                    "WASM string buffer address overflow",
                )?
                as usize;

        self.ensure_buffer_capacity(
            required,
        )?;

        let written =
            func.call(
                &mut self.store,
                (
                    index,
                    buffer_ptr,
                    BUFFER_SIZE as i32,
                ),
            )?;

        if written < 0
            || written as usize > BUFFER_SIZE
        {
            return Err(
                "invalid string length returned by WASM plugin"
                    .into(),
            );
        }

        let memory =
            self.memory.data(
                &self.store,
            );

        let start =
            buffer_ptr as usize;

        let end =
            start + written as usize;

        if end > memory.len() {
            return Err(
                "WASM string exceeds plugin memory"
                    .into(),
            );
        }

        Ok(String::from_utf8(
            memory[start..end].to_vec(),
        )?)
    }

    fn read_plugin_choice(
        &mut self,
        parameter_index: i32,
        choice_index: i32,
    ) -> Result<
        String,
        Box<dyn std::error::Error>,
    > {
        const BUFFER_SIZE: usize = 256;

        let func =
            self.plugin_parameter_choice
                .clone()
                .ok_or(
                    "WASM plugin declares a Choice parameter \
                     but does not export \
                     `plugin_parameter_choice`",
                )?;

        let buffer_ptr =
            self.buffer_ptr;

        let required =
            buffer_ptr
                .checked_add(
                    BUFFER_SIZE as i32,
                )
                .ok_or(
                    "WASM choice buffer address overflow",
                )?
                as usize;

        self.ensure_buffer_capacity(
            required,
        )?;

        let written =
            func.call(
                &mut self.store,
                (
                    parameter_index,
                    choice_index,
                    buffer_ptr,
                    BUFFER_SIZE as i32,
                ),
            )?;

        if written < 0
            || written as usize > BUFFER_SIZE
        {
            return Err(
                "invalid choice string length returned by WASM plugin"
                    .into(),
            );
        }

        let memory =
            self.memory.data(
                &self.store,
            );

        let start =
            buffer_ptr as usize;

        let end =
            start + written as usize;

        if end > memory.len() {
            return Err(
                "WASM choice string exceeds plugin memory"
                    .into(),
            );
        }

        Ok(String::from_utf8(
            memory[start..end].to_vec(),
        )?)
    }

    /// Read the plugin name from the WASM plugin.
    pub fn name(
        &mut self,
    ) -> Result<
        String,
        Box<dyn std::error::Error>,
    > {
        const BUFFER_SIZE: usize = 256;

        let buffer_ptr =
            self.buffer_ptr;

        let required =
            buffer_ptr
                .checked_add(
                    BUFFER_SIZE as i32,
                )
                .ok_or(
                    "WASM plugin name buffer address overflow",
                )?
                as usize;

        self.ensure_buffer_capacity(
            required,
        )?;

        let written =
            self.plugin_name.call(
                &mut self.store,
                (
                    buffer_ptr,
                    BUFFER_SIZE as i32,
                ),
            )?;

        if written < 0
            || written as usize > BUFFER_SIZE
        {
            return Err(
                "invalid plugin name length returned by WASM plugin"
                    .into(),
            );
        }

        let memory =
            self.memory.data(
                &self.store,
            );

        let start =
            buffer_ptr as usize;

        let end =
            start + written as usize;

        if end > memory.len() {
            return Err(
                "WASM plugin name exceeds plugin memory"
                    .into(),
            );
        }

        Ok(String::from_utf8(
            memory[start..end].to_vec(),
        )?)
    }

    /// Read the complete parameter metadata from the plugin.
    pub fn load_parameters(
        &mut self,
    ) -> Result<
        Vec<Parameter>,
        Box<dyn std::error::Error>,
    > {
        let count =
            match self
                .plugin_parameter_count
                .call(
                    &mut self.store,
                    (),
                )
            {
                Ok(count)
                    if count >= 0 =>
                {
                    count as usize
                }

                _ => {
                    return Err(
                        "failed to get plugin parameter count"
                            .into(),
                    );
                }
            };

        let mut parameters =
            Vec::with_capacity(count);

        let mut parameter_ids =
            HashSet::new();

        for parameter_index in 0..count {
            let index =
                parameter_index as i32;

            let id =
                self.read_plugin_string(
                    self.plugin_parameter_id.clone(),
                    index,
                )
                .map_err(|_| {
                    format!(
                        "failed to get parameter ID \
                         at index {}",
                        index
                    )
                })?;

            let name =
                self.read_plugin_string(
                    self.plugin_parameter_name.clone(),
                    index,
                )
                .map_err(|_| {
                    format!(
                        "failed to get parameter name \
                         at index {}",
                        index
                    )
                })?;

            let label =
                self.read_plugin_string(
                    self.plugin_parameter_label.clone(),
                    index,
                )
                .map_err(|_| {
                    format!(
                        "failed to get parameter label \
                         at index {}",
                        index
                    )
                })?;

            if !parameter_ids.insert(id.clone()) {
                return Err(
                    format!(
                        "duplicate parameter ID: {}",
                        id
                    )
                    .into(),
                );
            }

            let min =
                self.plugin_parameter_min
                    .call(
                        &mut self.store,
                        (index,),
                    )
                    .map_err(|_| {
                        format!(
                            "failed to get parameter min \
                             at index {}",
                            index
                        )
                    })?;

            let max =
                self.plugin_parameter_max
                    .call(
                        &mut self.store,
                        (index,),
                    )
                    .map_err(|_| {
                        format!(
                            "failed to get parameter max \
                             at index {}",
                            index
                        )
                    })?;

            let default =
                self.plugin_parameter_default
                    .call(
                        &mut self.store,
                        (index,),
                    )
                    .map_err(|_| {
                        format!(
                            "failed to get parameter default \
                             at index {}",
                            index
                        )
                    })?;

            let step =
                match self
                    .plugin_parameter_step
                    .call(
                        &mut self.store,
                        (index,),
                    )
                {
                    Ok(value)
                        if value > 0.0 =>
                    {
                        Some(value)
                    }

                    _ => None,
                };

            let flags =
                self.plugin_parameter_flags
                    .call(
                        &mut self.store,
                        (index,),
                    )
                    .map_err(|_| {
                        format!(
                            "failed to get parameter flags \
                             at index {}",
                            index
                        )
                    })?;

            let value_normalized =
                self.plugin_parameter_get
                    .call(
                        &mut self.store,
                        (index,),
                    )
                    .map_err(|_| {
                        format!(
                            "failed to get parameter value \
                             at index {}",
                            index
                        )
                    })?
                    .clamp(0.0, 1.0);

            let automatable =
                (flags & FLAG_AUTOMATABLE) != 0;

            let type_flags =
                flags
                    & (
                        FLAG_INT
                            | FLAG_BOOL
                            | FLAG_CHOICE
                    );

            let kind_and_default =
                match type_flags {
                    0 => {
                        (
                            ParameterKind::Float {
                                min,
                                max,
                                step,
                            },
                            ParameterValue::Float(
                                default,
                            ),
                        )
                    }

                    FLAG_INT => {
                        let int_min =
                            min.round() as i32;

                        let int_max =
                            max.round() as i32;

                        let int_step =
                            step
                                .unwrap_or(1.0)
                                .round()
                                .max(1.0)
                                as i32;

                        (
                            ParameterKind::Int {
                                min: int_min,
                                max: int_max,
                                step: int_step,
                            },
                            ParameterValue::Int(
                                default.round() as i32,
                            ),
                        )
                    }

                    FLAG_BOOL => {
                        (
                            ParameterKind::Bool,
                            ParameterValue::Bool(
                                default >= 0.5,
                            ),
                        )
                    }

                    FLAG_CHOICE => {
                        let choice_count =
                            if max >= 0.0 {
                                max.round() as usize + 1
                            } else {
                                0
                            };

                        if choice_count == 0 {
                            return Err(
                                format!(
                                    "Choice parameter '{}' \
                                     has no choices",
                                    id
                                )
                                .into(),
                            );
                        }

                        let mut choices =
                            Vec::with_capacity(
                                choice_count,
                            );

                        for choice_index
                            in 0..choice_count
                        {
                            choices.push(
                                self.read_plugin_choice(
                                    index,
                                    choice_index as i32,
                                )?,
                            );
                        }

                        let default_index =
                            default
                                .round()
                                .max(0.0)
                                as u32;

                        (
                            ParameterKind::Choice {
                                choices,
                            },
                            ParameterValue::Choice(
                                default_index,
                            ),
                        )
                    }

                    _ => {
                        return Err(
                            format!(
                                "parameter '{}' \
                                 has invalid type flags: {}",
                                id,
                                flags
                            )
                            .into(),
                        );
                    }
                };

            let (
                kind,
                default_value,
            ) = kind_and_default;

            let mut parameter =
                Parameter {
                    id,
                    name,
                    label,
                    kind,
                    default: default_value,
                    value: ParameterValue::Float(0.0),
                    automatable,
                };

            let value =
                parameter
                    .value_from_normalized(
                        value_normalized,
                    );

            parameter.set_value(value);

            if !parameter.is_valid() {
                return Err(
                    format!(
                        "invalid parameter: {}",
                        parameter.id
                    )
                    .into(),
                );
            }

            parameters.push(parameter);
        }

        Ok(parameters)
    }

    /// Write the parameter defaults into the WASM plugin
    /// and synchronize the resulting values back to the host.
    pub fn initialize_parameter_defaults(
        &mut self,
        parameters: &mut [Parameter],
    ) {
        for index in 0..parameters.len() {
            let normalized =
                parameters[index]
                    .default_normalized_value();

            if self
                .plugin_parameter_set
                .call(
                    &mut self.store,
                    (
                        index as i32,
                        normalized,
                    ),
                )
                .is_err()
            {
                continue;
            }

            if let Ok(actual_normalized) =
                self.plugin_parameter_get
                    .call(
                        &mut self.store,
                        (index as i32,),
                    )
            {
                parameters[index]
                    .set_normalized(
                        actual_normalized,
                    );
            }
        }
    }

    /// Set a host-side parameter value in the WASM plugin.
    pub fn set_parameter(
        &mut self,
        index: usize,
        parameter: &mut Parameter,
        value: ParameterValue,
    ) {
        parameter.set_value(value);

        let normalized =
            parameter.normalized_value();

        let _ =
            self.plugin_parameter_set.call(
                &mut self.store,
                (
                    index as i32,
                    normalized,
                ),
            );
    }

    /// Process one audio block through the WASM plugin.
    /// Multi-bus ABI v2: interleaved input frames followed by interleaved
    /// output frames. Channel order is bus order, then channel order.
    /// Returns false when the plugin only supports the legacy ABI.
    pub fn process_buses(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
    ) -> bool {
        let Some(func) = self.plugin_process_buses.clone() else {
            return false;
        };
        let total = match input.len().checked_add(output.len()) {
            Some(total) => total,
            None => return false,
        };
        // The JUCE bridge exposes a fixed 16,384-float ABI buffer.
        if total > 16_384 || frames > i32::MAX as usize { return false; }
        let bytes = match total.checked_mul(4) {
            Some(bytes) => bytes,
            None => return false,
        };
        let offset = self.buffer_ptr as usize;
        let end = match offset.checked_add(bytes) {
            Some(end) => end,
            None => return false,
        };
        if self.ensure_buffer_capacity(end).is_err() { return false; }
        {
            let memory = self.memory.data_mut(&mut self.store);
            if end > memory.len() { return false; }
            for (i, value) in input.iter().enumerate() {
                let start = offset + i * 4;
                memory[start..start + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        if func.call(&mut self.store, (self.buffer_ptr, frames as i32)).is_err() {
            return false;
        }
        let memory = self.memory.data(&self.store);
        let start = offset + input.len() * 4;
        copy_bytes_to_f32(&memory[start..end], output);
        true
    }

    pub fn process(
        &mut self,
        buffer: &mut [f32],
    ) {
        let byte_len =
            match buffer
                .len()
                .checked_mul(
                    std::mem::size_of::<f32>(),
                )
            {
                Some(value) => value,
                None => return,
            };

        let offset =
            self.buffer_ptr as usize;

        let required =
            match offset.checked_add(
                byte_len,
            ) {
                Some(value) => value,
                None => return,
            };

        if self
            .ensure_buffer_capacity(
                required,
            )
            .is_err()
        {
            return;
        }

        let memory =
            self.memory.data_mut(
                &mut self.store,
            );

        let end =
            offset + byte_len;

        if end > memory.len() {
            return;
        }

        for (index, value) in
            buffer.iter().enumerate()
        {
            let bytes =
                value.to_le_bytes();

            let byte_offset =
                offset + index * 4;

            memory[byte_offset..byte_offset + 4]
                .copy_from_slice(&bytes);
        }

        if self
            .plugin_process
            .call(
                &mut self.store,
                (
                    self.buffer_ptr,
                    buffer.len() as i32,
                ),
            )
            .is_err()
        {
            return;
        }

        let memory =
            self.memory.data(
                &self.store,
            );

        let processed =
            &memory[offset..end];

        copy_bytes_to_f32(
            processed,
            buffer,
        );
    }
}

fn copy_bytes_to_f32(
    bytes: &[u8],
    output: &mut [f32],
) {
    let expected_len =
        output.len() * std::mem::size_of::<f32>();

    if bytes.len() != expected_len {
        return;
    }

    for (index, chunk) in
        bytes.chunks_exact(4).enumerate()
    {
        output[index] =
            f32::from_le_bytes([
                chunk[0],
                chunk[1],
                chunk[2],
                chunk[3],
            ]);
    }
}