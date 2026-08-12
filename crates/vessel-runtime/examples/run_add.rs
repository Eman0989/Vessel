use vessel_runtime::WasmRuntime;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = WasmRuntime::new();

    let result = runtime.invoke_i32_binary(ADD_MODULE.as_bytes(), "add", 20, 22)?;

    println!("VESSEL runtime result: 20 + 22 = {result}");

    Ok(())
}
