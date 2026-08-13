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

#[test]
fn epoch_deadline_stops_long_running_execution() {
    use std::time::Duration;

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
        fuel: u64::MAX,
        timeout: Duration::from_millis(50),
        ..RuntimeLimits::default()
    })
    .unwrap();

    let result = runtime.invoke_i32_binary(SPIN_MODULE.as_bytes(), "add", 1, 2);

    match result {
        Err(RuntimeError::Timeout { timeout_ms, source }) => {
            assert_eq!(timeout_ms, 50);

            assert_eq!(
                source.downcast_ref::<wasmtime::Trap>(),
                Some(&wasmtime::Trap::Interrupt),
            );
        }

        other => {
            panic!("expected timeout interrupt, got {other:?}");
        }
    }
}

#[test]
fn runtime_remains_usable_after_timeout() {
    use std::time::Duration;

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

    const ADD_MODULE_AFTER_TIMEOUT: &str = r#"
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

    let runtime = WasmRuntime::with_limits(RuntimeLimits {
        fuel: u64::MAX,
        timeout: Duration::from_millis(50),
        ..RuntimeLimits::default()
    })
    .unwrap();

    let timed_out = runtime.invoke_i32_binary(SPIN_MODULE.as_bytes(), "add", 1, 2);

    assert!(matches!(timed_out, Err(RuntimeError::Timeout { .. })));

    let result = runtime
        .invoke_i32_binary(ADD_MODULE_AFTER_TIMEOUT.as_bytes(), "add", 20, 22)
        .unwrap();

    assert_eq!(result, 42);
}

const WASI_ENVIRONMENT_COMPONENT: &str = r#"
(component
    (import "wasi:cli/environment@0.2.12"
        (instance
            (export "get-environment"
                (func
                    (result
                        (list
                            (tuple string string)
                        )
                    )
                )
            )

            (export "get-arguments"
                (func
                    (result (list string))
                )
            )

            (export "initial-cwd"
                (func
                    (result (option string))
                )
            )
        )
    )
)
"#;

#[test]
fn default_wasi_policy_exposes_no_environment() {
    let runtime = WasmRuntime::new();

    let environment = runtime
        .invoke_wasi_environment(WASI_ENVIRONMENT_COMPONENT.as_bytes())
        .unwrap();

    assert!(environment.is_empty());
}

#[test]
fn explicit_wasi_environment_is_visible() {
    use std::collections::BTreeMap;

    use vessel_policy::CapabilityPolicy;

    let mut environment = BTreeMap::new();

    environment.insert("VESSEL_MODE".to_string(), "sandbox".to_string());

    let policy = CapabilityPolicy {
        environment,
        ..CapabilityPolicy::deny_all()
    };

    let runtime = WasmRuntime::with_limits_and_policy(RuntimeLimits::default(), policy).unwrap();

    let environment = runtime
        .invoke_wasi_environment(WASI_ENVIRONMENT_COMPONENT.as_bytes())
        .unwrap();

    assert_eq!(
        environment,
        vec![("VESSEL_MODE".to_string(), "sandbox".to_string(),)]
    );
}

#[test]
fn invalid_preopen_fails_before_guest_execution() {
    use std::path::PathBuf;

    use vessel_policy::{CapabilityPolicy, DirectoryAccess, DirectoryCapability};

    let policy = CapabilityPolicy {
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from("/this/vessel/path/does/not/exist"),
            guest_path: "/data".to_string(),
            access: DirectoryAccess::ReadOnly,
        }],
        ..CapabilityPolicy::deny_all()
    };

    let runtime = WasmRuntime::with_limits_and_policy(RuntimeLimits::default(), policy).unwrap();

    let result = runtime.invoke_wasi_environment(WASI_ENVIRONMENT_COMPONENT.as_bytes());

    assert!(matches!(result, Err(RuntimeError::WasiContext(_))));
}

#[test]
fn default_policy_has_no_preopened_directories() {
    let runtime = WasmRuntime::new();

    let directories = runtime.wasi_preopened_directories().unwrap();

    assert!(directories.is_empty());
}

#[test]
fn explicit_directory_is_preopened_for_wasi() {
    use std::path::PathBuf;

    use vessel_policy::{CapabilityPolicy, DirectoryAccess, DirectoryCapability};

    let host_directory = tempfile::tempdir().unwrap();

    let policy = CapabilityPolicy {
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from(host_directory.path()),
            guest_path: "/data".to_string(),
            access: DirectoryAccess::ReadOnly,
        }],
        ..CapabilityPolicy::deny_all()
    };

    let runtime = WasmRuntime::with_limits_and_policy(RuntimeLimits::default(), policy).unwrap();

    let directories = runtime.wasi_preopened_directories().unwrap();

    assert_eq!(directories, vec!["/data".to_string()]);
}

#[test]
fn read_only_preopen_rejects_file_creation() {
    use std::path::PathBuf;

    use vessel_policy::{CapabilityPolicy, DirectoryAccess, DirectoryCapability};

    let host_directory = tempfile::tempdir().unwrap();

    let policy = CapabilityPolicy {
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from(host_directory.path()),
            guest_path: "/data".to_string(),
            access: DirectoryAccess::ReadOnly,
        }],
        ..CapabilityPolicy::deny_all()
    };

    let runtime = WasmRuntime::with_limits_and_policy(RuntimeLimits::default(), policy).unwrap();

    let created = runtime.wasi_can_create_file("blocked.txt").unwrap();

    assert!(!created);

    assert!(!host_directory.path().join("blocked.txt").exists());
}

#[test]
fn read_write_preopen_allows_file_creation() {
    use std::path::PathBuf;

    use vessel_policy::{CapabilityPolicy, DirectoryAccess, DirectoryCapability};

    let host_directory = tempfile::tempdir().unwrap();

    let policy = CapabilityPolicy {
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from(host_directory.path()),
            guest_path: "/data".to_string(),
            access: DirectoryAccess::ReadWrite,
        }],
        ..CapabilityPolicy::deny_all()
    };

    let runtime = WasmRuntime::with_limits_and_policy(RuntimeLimits::default(), policy).unwrap();

    let created = runtime.wasi_can_create_file("allowed.txt").unwrap();

    assert!(created);

    assert!(host_directory.path().join("allowed.txt").exists());
}
