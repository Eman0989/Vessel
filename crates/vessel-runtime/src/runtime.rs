use crate::{RuntimeError, RuntimeLimits, bindings::VesselWorkload, limits::StoreState};
use wasmtime::component::{Component, Linker as ComponentLinker};
use wasmtime::{Config, Engine, Instance, Module, Store};

#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
    limits: RuntimeLimits,
}

impl WasmRuntime {
    pub fn new() -> Self {
        Self::with_limits(RuntimeLimits::default())
            .expect("default VESSEL Wasmtime configuration must be valid")
    }

    pub fn with_limits(limits: RuntimeLimits) -> Result<Self, RuntimeError> {
        let mut config = Config::new();

        config.consume_fuel(true);

        let engine = Engine::new(&config).map_err(RuntimeError::Initialize)?;

        Ok(Self { engine, limits })
    }

    fn new_store(&self) -> Result<Store<StoreState>, RuntimeError> {
        let state = StoreState::new(self.limits);

        let mut store = Store::new(&self.engine, state);

        store.limiter(|state| &mut state.limits);

        store
            .set_fuel(self.limits.fuel)
            .map_err(RuntimeError::Budget)?;

        Ok(store)
    }

    pub fn invoke_i32_binary(
        &self,
        module_bytes: &[u8],
        export: &str,
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let module = Module::new(&self.engine, module_bytes).map_err(RuntimeError::Compile)?;

        let mut store = self.new_store()?;

        let instance =
            Instance::new(&mut store, &module, &[]).map_err(RuntimeError::Instantiate)?;

        let function = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, export)
            .map_err(|source| RuntimeError::Export {
                export: export.to_string(),
                source,
            })?;

        function
            .call(&mut store, (lhs, rhs))
            .map_err(RuntimeError::Execute)
    }

    pub fn invoke_component_i32_binary(
        &self,
        component_bytes: &[u8],
        export: &str,
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let component = Component::new(&self.engine, component_bytes)
            .map_err(RuntimeError::ComponentCompile)?;

        let linker = ComponentLinker::<StoreState>::new(&self.engine);

        let mut store = self.new_store()?;

        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(RuntimeError::ComponentInstantiate)?;

        let function = instance
            .get_typed_func::<(i32, i32), (i32,)>(&mut store, export)
            .map_err(|source| RuntimeError::ComponentExport {
                export: export.to_string(),
                source,
            })?;

        let (result,) = function
            .call(&mut store, (lhs, rhs))
            .map_err(RuntimeError::ComponentExecute)?;

        Ok(result)
    }

    pub fn invoke_wit_bound_add(
        &self,
        component_bytes: &[u8],
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let component = Component::new(&self.engine, component_bytes)
            .map_err(RuntimeError::ComponentCompile)?;

        let linker = ComponentLinker::<StoreState>::new(&self.engine);

        let mut store = self.new_store()?;

        let bindings = VesselWorkload::instantiate(&mut store, &component, &linker)
            .map_err(RuntimeError::ComponentInstantiate)?;

        bindings
            .call_add(&mut store, lhs, rhs)
            .map_err(RuntimeError::ComponentExecute)
    }

    pub fn limits(&self) -> RuntimeLimits {
        self.limits
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}
