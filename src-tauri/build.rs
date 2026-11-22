fn main() {
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
