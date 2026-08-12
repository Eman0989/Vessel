use vessel_runtime::{RuntimeError, RuntimeLimits, WasmRuntime};

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

const ADD_COMPONENT: &str = r#"
(component
    (core module $implementation
        (func (export "add")
            (param i32 i32)
            (result i32)

            local.get 0
            local.get 1
            i32.add
        )
    )

    (core instance $instance
        (instantiate $implementation)
    )

    (func (export "add")
        (param "a" s32)
        (param "b" s32)
        (result s32)

        (canon lift
            (core func $instance "add")
        )
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

#[test]
fn executes_component_function() {
    let runtime = WasmRuntime::new();

    let result = runtime
        .invoke_component_i32_binary(ADD_COMPONENT.as_bytes(), "add", 20, 22)
        .unwrap();

    assert_eq!(result, 42);
}

#[test]
fn component_supports_negative_values() {
    let runtime = WasmRuntime::new();

    let result = runtime
        .invoke_component_i32_binary(ADD_COMPONENT.as_bytes(), "add", -20, 8)
        .unwrap();

    assert_eq!(result, -12);
}

#[test]
fn component_rejects_missing_export() {
    let runtime = WasmRuntime::new();

    let result = runtime.invoke_component_i32_binary(ADD_COMPONENT.as_bytes(), "missing", 1, 2);

    assert!(matches!(result, Err(RuntimeError::ComponentExport { .. })));
}

#[test]
fn rejects_invalid_component() {
    let runtime = WasmRuntime::new();

    let invalid = b"not a WebAssembly component";

    let result = runtime.invoke_component_i32_binary(invalid, "add", 1, 2);

    assert!(matches!(result, Err(RuntimeError::ComponentCompile(_))));
}

#[test]
fn executes_component_through_wit_bindings() {
    let runtime = WasmRuntime::new();

    let result = runtime
        .invoke_wit_bound_add(ADD_COMPONENT.as_bytes(), 20, 22)
        .unwrap();

    assert_eq!(result, 42);
}

#[test]
fn wit_bindings_reject_incompatible_component() {
    const WRONG_COMPONENT: &str = r#"
(component
    (core module $implementation
        (func (export "subtract")
            (param i32 i32)
            (result i32)

            local.get 0
            local.get 1
            i32.sub
        )
    )

    (core instance $instance
        (instantiate $implementation)
    )

    (func (export "subtract")
        (param "a" s32)
        (param "b" s32)
        (result s32)

        (canon lift
            (core func $instance "subtract")
        )
    )
)
"#;

    let runtime = WasmRuntime::new();

    let result = runtime.invoke_wit_bound_add(WRONG_COMPONENT.as_bytes(), 20, 22);

    assert!(matches!(result, Err(RuntimeError::ComponentInstantiate(_))));
}

#[test]
fn fuel_stops_infinite_execution() {
    const SPIN_MODULE: &str = r#"
(module
    (func (export "add")
        (param i32 i32)
        (result i32)

        (loop $spin
            br $spin
        )

        i32.const 0
    )
)
"#;

    let runtime = WasmRuntime::with_limits(RuntimeLimits {
        fuel: 1_000,
        ..RuntimeLimits::default()
    })
    .unwrap();

    let result = runtime.invoke_i32_binary(SPIN_MODULE.as_bytes(), "add", 1, 2);

    match result {
        Err(RuntimeError::Execute(error)) => {
            assert_eq!(
                error.downcast_ref::<wasmtime::Trap>(),
                Some(&wasmtime::Trap::OutOfFuel),
            );
        }

        other => {
            panic!("expected out-of-fuel trap, got {other:?}");
        }
    }
}

#[test]
fn memory_limit_rejects_oversized_module() {
    const MEMORY_MODULE: &str = r#"
(module
    (memory 2)

    (func (export "add")
        (param i32 i32)
        (result i32)

        local.get 0
        local.get 1
        i32.add
    )
)
"#;

    let runtime = WasmRuntime::with_limits(RuntimeLimits {
        memory_bytes: 64 * 1024,
        ..RuntimeLimits::default()
    })
    .unwrap();

    let result = runtime.invoke_i32_binary(MEMORY_MODULE.as_bytes(), "add", 20, 22);

    assert!(matches!(result, Err(RuntimeError::Instantiate(_))));
}
