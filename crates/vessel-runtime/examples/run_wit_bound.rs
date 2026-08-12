use vessel_runtime::WasmRuntime;

const VESSEL_COMPONENT: &str = r#"
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = WasmRuntime::new();

    let result = runtime.invoke_wit_bound_add(VESSEL_COMPONENT.as_bytes(), 20, 22)?;

    println!("VESSEL WIT-bound result: 20 + 22 = {result}");

    Ok(())
}
