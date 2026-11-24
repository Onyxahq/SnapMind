// pyke 的 Windows 版 ONNX Runtime 静态库总是带着 DirectML，会让程序在启动时就依赖
// DirectML.dll / D3D12.dll / DXCORE.dll。我们只用 CPU 推理，把它们改成延迟加载：
// 真正用到 GPU 才去加载，平时缺了也能正常启动。
//
// 注意：rustc-link-arg 只对本包的二进制、测试生效；应用（src-tauri）在自己的 build.rs 里做同样的设置。
fn main() {
    link_clang_rt_on_macos();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        for dll in ["DirectML.dll", "D3D12.dll", "DXCORE.dll"] {
            println!("cargo:rustc-link-arg=/DELAYLOAD:{dll}");
        }
        println!("cargo:rustc-link-arg=delayimp.lib");
        // 没用到 ORT 的测试二进制里找不到这些导入，链接器会警告 LNK4199，无害
        println!("cargo:rustc-link-arg=/IGNORE:4199");
    }
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
