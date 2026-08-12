use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to initialize WebAssembly runtime: {0}")]
    Initialize(#[source] wasmtime::Error),

    #[error("failed to configure WebAssembly execution budget: {0}")]
    Budget(#[source] wasmtime::Error),

    #[error("WebAssembly execution exceeded its {timeout_ms} ms deadline")]
    Timeout {
        timeout_ms: u64,

        #[source]
        source: wasmtime::Error,
    },

    #[error("failed to compile WebAssembly module: {0}")]
    Compile(#[source] wasmtime::Error),

    #[error("failed to instantiate WebAssembly module: {0}")]
    Instantiate(#[source] wasmtime::Error),

    #[error("failed to resolve export `{export}`: {source}")]
    Export {
        export: String,

        #[source]
        source: wasmtime::Error,
    },

    #[error("WebAssembly execution failed: {0}")]
    Execute(#[source] wasmtime::Error),

    #[error("failed to compile WebAssembly component: {0}")]
    ComponentCompile(#[source] wasmtime::Error),

    #[error("failed to instantiate WebAssembly component: {0}")]
    ComponentInstantiate(#[source] wasmtime::Error),

    #[error("failed to resolve component export `{export}`: {source}")]
    ComponentExport {
        export: String,

        #[source]
        source: wasmtime::Error,
    },

    #[error("WebAssembly component execution failed: {0}")]
    ComponentExecute(#[source] wasmtime::Error),
}
