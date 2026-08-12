use vessel_runtime::{RuntimeError, WasmRuntime};

const ADD_MODULE: &str = r#"
(module
    (func (export "add")
        (param i32 i32)
        (result i32)

        local.get 0
        local.get 1
        i32.add
    )
)
"#;

#[test]
fn executes_webassembly_function() {
    let runtime = WasmRuntime::new();

    let result = runtime
        .invoke_i32_binary(ADD_MODULE.as_bytes(), "add", 20, 22)
        .unwrap();

    assert_eq!(result, 42);
}

#[test]
fn supports_negative_values() {
    let runtime = WasmRuntime::new();

    let result = runtime
        .invoke_i32_binary(ADD_MODULE.as_bytes(), "add", -10, 4)
        .unwrap();

    assert_eq!(result, -6);
}

#[test]
fn rejects_missing_export() {
    let runtime = WasmRuntime::new();

    let result = runtime.invoke_i32_binary(ADD_MODULE.as_bytes(), "does_not_exist", 1, 2);

    assert!(matches!(result, Err(RuntimeError::Export { .. })));
}

#[test]
fn rejects_invalid_webassembly() {
    let runtime = WasmRuntime::new();

    let invalid = b"this is definitely not WebAssembly";

    let result = runtime.invoke_i32_binary(invalid, "add", 1, 2);

    assert!(matches!(result, Err(RuntimeError::Compile(_))));
}
