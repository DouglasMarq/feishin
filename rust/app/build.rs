fn main() -> Result<(), slint_build::CompileError> {
    let debug_build = std::env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-dark".into())
        .with_debug_info(debug_build);
    slint_build::compile_with_config("ui/app.slint", config)
}
