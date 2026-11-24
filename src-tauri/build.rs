fn main() {
    link_clang_rt_on_macos();
    // 本地 OCR 用的 ONNX Runtime 静态库带着 DirectML，会让程序启动时就依赖 DirectML.dll / D3D12.dll /
    // DXCORE.dll。只用 CPU 推理，所以改成延迟加载（和 crates/snapmind-ocr/build.rs 同样的设置）。
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        for dll in ["DirectML.dll", "D3D12.dll", "DXCORE.dll"] {
            println!("cargo:rustc-link-arg=/DELAYLOAD:{dll}");
        }
        println!("cargo:rustc-link-arg=delayimp.lib");
        println!("cargo:rustc-link-arg=/IGNORE:4199");
    }
    tauri_build::build()
}

/// macOS：pyke 的 ONNX Runtime 静态库（CoreML 部分）用到 clang 运行时里的 `__isPlatformVersionAtLeast`，
/// 而 rustc 链接时带 `-nodefaultlibs`，不会自动链上 clang 运行时，要手动把 libclang_rt.osx.a 加进去。
fn link_clang_rt_on_macos() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = std::process::Command::new("xcrun").args(["clang", "-print-resource-dir"]).output();
    let Ok(out) = out else { return };
    let dir = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join("lib/darwin");
    let lib = dir.join("libclang_rt.osx.a");
    if lib.exists() {
        println!("cargo:rustc-link-arg={}", lib.display());
    }
}
