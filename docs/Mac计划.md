# SnapMind macOS 计划 v1.0

> 2026-11-25 写成。总计划见 [编程计划.md](编程计划.md)，进度记在 [进度.md](进度.md) 的「Mac」一行和日志里。
> Mac 相关任务编号用 **MAC-x.y**，每做完一个在本文打勾，并在进度日志写一行。

---

## 1. 目标与原则

- **同一份代码，Windows 和 Mac 两个平台都要能用。** Windows 是已有主力平台，Mac 的改动不能让 Windows 退步；反过来，以后给 Windows 加功能也要想清楚 Mac 上怎么做。
- **平台差异只放在 `crates/snapmind-platform`**（以及 `src-tauri` 里少数必须拿窗口句柄的地方，统一走 `wm::native_handle`）。每个模块的写法固定：
  ```rust
  #[cfg(windows)] mod imp { … }                         // Windows
  #[cfg(target_os = "macos")] mod imp { … }             // macOS
  #[cfg(not(any(windows, target_os = "macos")))] mod imp { … } // 其他（Linux 以后做）
  ```
  对外的函数签名两边一致，上层代码不写平台判断。
- **最低 macOS 14**（ScreenCaptureKit 截图接口从 14 开始有）；新系统上有更快的接口就用 `objc2::available!(macos = x.y)` 判断后走快路径，旧系统走兼容路径。
- **先 Apple 芯片（aarch64）**，Intel / Universal 包等有需要再加。
- **Mac 用户习惯**：快捷键里 Ctrl 的地方 Mac 上用 ⌘（判断 `e.ctrlKey || e.metaKey`，界面文字显示 ⌘）；托盘工具不在程序坞显示图标（`ActivationPolicy::Accessory`）。

## 2. Windows ↔ macOS 对照

| 能力 | Windows | macOS | 状态 |
|---|---|---|---|
| 截屏 | GDI BitBlt | ScreenCaptureKit：15.2+ `captureImageInRect`，14 ~ 15.1 按显示器 + `SCScreenshotManager` | ✅ |
| 截屏权限 | 不需要 | 屏幕录制（TCC）：`CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`，设置 → 通用 → 系统权限 | ✅ |
| 遮罩 / 贴图窗口 | Win32 扩展样式 | AppKit：`collectionBehavior`、`NSScreenSaverWindowLevel`、`orderFrontRegardless`、`sharingType` | ✅ |
| 坐标 | 物理像素 | tao 约定：显示器位置 = 点 × 该显示器缩放；放窗口用 Logical 坐标 | ✅（混合缩放未测） |
| 剪贴板 文字 / HTML / 图片 | Win32 多格式 | arboard（文字、HTML、图片） | 基本可用；「粘贴成文件」未做 |
| 密钥 | Credential Manager | Keychain（keyring 自带） | ✅ |
| 系统界面语言 | `GetUserPreferredUILanguages` | `NSLocale.preferredLanguages`（地区：`NSLocale.currentLocale.regionCode`） | ✅ |
| 窗口吸附 | EnumWindows + UI Automation | `CGWindowListCopyWindowInfo` + AXUIElement（辅助功能权限） | ✅ 代码完成，待真机验收 |
| 划词 | UI Automation + 模拟 Ctrl+C | AX `kAXSelectedTextAttribute` + 模拟 ⌘C（CGEvent） | ❌ MAC-2.2 |
| 鼠标微移 / 位置 / 滚轮 | SendInput | CGEvent / `CGWarpMouseCursorPosition` | ✅ 代码完成，待真机验收 |
| 长截图自动滚动 | 发滚轮消息 | CGEvent 滚轮直接投给目标进程（辅助功能权限） | ✅ 代码完成，待真机验收 |
| 录屏 | 逐帧截 + Media Foundation + WASAPI | SCStream（视频 + 系统声音）+ AVAssetWriter | ❌ MAC-3.1 |
| 开机自启 | 注册表（autostart 插件） | LaunchAgent（同一插件） | 未验证 MAC-1.4 |
| 安装包 / 更新 | NSIS + updater（windows-x86_64） | dmg + updater（darwin-aarch64） | ❌ MAC-3.2 |
| 签名 | 无 | 本机自签名「SnapMind Local Signing」；对外发布需 Developer ID + 公证 | 本机 ✅ / 发布待用户 |

## 3. 分期任务

### 一期：能用（✅ 2026-11-24 ~ 25）

- [x] **MAC-1.1** 编译打包：objc2 依赖、`hwnd()` 换成 `wm::native_handle`、ort 链接 `libclang_rt.osx.a`、`macOSPrivateApi`、图标、最低 14.0。
- [x] **MAC-1.2** 截屏 + 遮罩 + 权限引导：ScreenCaptureKit 截屏；遮罩盖住菜单栏、跨桌面、不抢焦点；设置 → 通用 → 系统权限（屏幕录制状态、去授权、重启）；固定签名身份，重编不丢授权；`scripts/mac-build.sh`。
  - 性能：15.2+ 改用 `captureImageInRect`，抓屏 96 ~ 248 ms → 实测 13 ~ 48 ms（应用内基准），真实使用约 100 ms；按键 → 遮罩出现 280 ~ 380 ms → 160 ~ 260 ms。

### 一期收尾：打磨（下一步）

- [x] **MAC-1.3** 显示语言跟随系统：Mac 上用 `NSLocale.preferredLanguages` 取首选语言。验收：从访达启动、系统语言中文时界面是中文。
- [ ] **MAC-1.4** 逐项真机点验已有功能：标注、贴图、复制图片 / 文字 / 表格 HTML、保存、本地 OCR（ONNX Runtime 在 Mac 上实际识别一次）、翻译、开机自启、悬浮球。发现问题当场修。
  - ✅ 本地 OCR：ort 默认在 macOS 挂 CoreML 导致乱码，改为只用 CPU；评测和 Windows 一致，平均 78 ms。
  - 待用户点验（需要截图交互）：标注、贴图、复制、保存、应用内识别、翻译、开机自启、悬浮球。
- [x] **MAC-1.5** Mac 键盘习惯：默认截图快捷键改成 Mac 上好按的组合（Windows 默认 `Shift+F1` 不变，只改 Mac 默认值）；界面里的快捷键提示显示 ⌘ / ⌥ / ⇧；排查所有只认 Ctrl 的按键处理。
- [ ] **MAC-1.6** 遮罩显示提速：按键 → 冻结帧画好里，窗口显示后页面拿到 1920×1080 BMP（约 8 MB）再画出来还要 60 ~ 150 ms。方向：缩小传输（压缩或共享内存）、提前准备好画布。验收：按键 → 遮罩出现 ≤ 150 ms。
  - 已测：本机 5K 屏一帧 59 MB，像素搬运合计 14 ~ 28 ms；「窗口显示 → 遮罩出现」55 ~ 155 ms 是大头，要屏幕亮着时测（测速补丁 `.git/mac16-bench.patch`）。
- [ ] **MAC-1.7** 高分屏和多显示器：Retina（缩放 2）、外接 1 倍屏混用时截图位置、清晰度、遮罩覆盖都正确。需要用户有对应显示器环境。

### 二期：和 Windows 功能对齐（交互类）

- [ ] **MAC-2.1** 窗口吸附：`CGWindowListCopyWindowInfo` 拍顶层窗口快照（排除本进程），AXUIElement 往里找控件做吸附链。权限页加「辅助功能」一行（`AXIsProcessTrustedWithOptions`）。验收：截图时悬停自动框出窗口和按钮，滚轮切层级。
  - ✅ 第一步：顶层窗口快照（CGWindowListCopyWindowInfo，无需权限），悬停框出整个窗口。
  - ✅ 第二步：AX 控件吸附链 + 权限页「辅助功能」一行；终端里真机测过。待用户给 SnapMind 授权后在 Safari / Chrome / 访达 里悬停验收。
- [ ] **MAC-2.2** 划词：先用 AX 取选中文字，取不到再模拟 ⌘C（存剪贴板 → 发 ⌘C → 等 `changeCount` 变 → 读 → 还原）。验收：Safari、Chrome、备忘录、VS Code 里划词翻译可用。
- [ ] **MAC-2.3** 鼠标与滚轮：`cursor.rs` 的 nudge / position / 左键状态 / 发滚轮用 CGEvent 实现；长截图自动滚动可用。
  - ✅ 代码完成并真机测过指针移动和滚轮投递；待用户在时用长截图核对滚动量与横向方向，并在系统设置里给 SnapMind 辅助功能授权。

### 三期：录屏与发布

- [ ] **MAC-3.1** 录屏：SCStream 取视频帧和系统声音，AVAssetWriter 写 H.264 + AAC 的 MP4；GIF 沿用现有编码。
- [ ] **MAC-3.2** 发布：`scripts/release.mjs` 加 Mac 分支（dmg、`latest.json` 的 `darwin-aarch64`），更新器在 Mac 上能装新版。**需要用户决定**：是否购买 Apple Developer 账号做 Developer ID 签名和公证——不做的话，别人下载后要右键打开或在「隐私与安全性」里放行，且每台机器都要重新授权屏幕录制。

## 4. 在 Mac 上开发与验证

- **打包安装**：`scripts/mac-build.sh`（构建 → 用「SnapMind Local Signing」签名 → 装到 `/Applications` → 删掉 target 里的 .app → 启动）。本机没有全局 pnpm 时可用 `npx --yes pnpm@10 …` 代替。
- **测试**：`cargo test --workspace`、`pnpm test`、`pnpm check`、`node scripts/i18n.mjs check`。
- **日志**：`~/Library/Application Support/SnapMind/logs/snapmind.log`；截图耗时看 `[capture #n]` 几行。
- **权限**：Claude / 终端进程没有屏幕录制权限，平台层的真截屏测试（`#[ignore]`）在终端里跑会失败。要测真实耗时，临时在 `lib.rs` 的 setup 里加一个读环境变量的基准代码，用 `open -n --env SNAPMIND_BENCH=1 target/release/bundle/macos/SnapMind.app` 启动（应用自己有权限），结果写进日志，测完删掉，不提交。
- **授权失效排查**：`sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" "select hex(csreq) from access where service='kTCCServiceScreenCapture' and client='app.snapmind.desktop'"`，用 `csreq -r - -t` 解出要求，和 `codesign -d -r- /Applications/SnapMind.app` 对比。
- **Windows 没法在 Mac 上编译验证**：在 Mac 上动了 Windows 分支或共用代码，汇报时写明「Windows 未编译验证」，下次在 Windows 机器上先编译、跑测试、打包。

## 5. 风险

| 风险 | 对策 |
|---|---|
| 混合缩放（Retina + 1 倍外接屏）坐标错位 | 所有换算集中在 `capture.rs` 的 `scale_at` 和 `overlay.rs` 的 `place`；MAC-1.7 真机验 |
| 系统周期性弹「继续允许录屏」提醒 | 系统行为，无法关闭；提醒不影响截图 |
| 辅助功能权限和屏幕录制一样绑签名 | 继续用固定的本机证书签名；对外发布要 Developer ID |
| 苹果新系统改 / 废弃截屏接口 | 截屏只有一个入口 `capture::capture_rect`，按 `available!` 分路径 |
