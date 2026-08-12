use crate::RuntimeError;
use wasmtime::{Engine, Instance, Module, Store};

#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    pub fn invoke_i32_binary(
        &self,
        module_bytes: &[u8],
        export: &str,
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let module = Module::new(&self.engine, module_bytes).map_err(RuntimeError::Compile)?;

        let mut store = Store::new(&self.engine, ());

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

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}
