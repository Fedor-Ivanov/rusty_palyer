//! Вшивает иконку в .exe при сборке под Windows (в том числе кросс-сборке с macOS через MinGW).

fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");

    // build.rs выполняется на машине сборки, поэтому смотрим на целевую ОС, а не на cfg!(windows)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app_icon.ico");
        resource
            .compile()
            .expect("could not embed the icon into the .exe");
    }
}
