use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
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
}
