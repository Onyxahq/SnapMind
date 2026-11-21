# SnapMind

屏幕上看到外语时，用最少的操作看懂它，需要时还能留住它。

带 AI 的截图、贴图、OCR 与翻译工具。技术路线是 Tauri 2，先做 Windows，再做 Linux（KDE Plasma / Wayland）。

## 当前状态

工程骨架阶段（M0）。进度见 [docs/进度.md](docs/进度.md)。

开发：`pnpm install` 后运行 `pnpm tauri dev`；打包并安装到本机：`pnpm release`（可加 `patch` / `minor` 升版本号，`--launch` 安装后启动），产物在 `target/release/bundle/nsis/`。

- [docs/需求.md](docs/需求.md)：需求文档（v0.3）
- [design/prototype/](design/prototype/)：可操作原型源码，包括截图编辑态、设置、贴图窗口、云母浅色 / 深夜视觉稿和设计规范。原型画布托管在 claude.ai 上，这里是备份。
