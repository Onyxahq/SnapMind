<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" height="96" alt="SnapMind Logo" />
</p>

<h1 align="center">SnapMind</h1>

<p align="center">
  <strong>带 AI 的桌面截图、贴图、本地 OCR、翻译、标注与录屏工具</strong><br />
  屏幕上看到外语或内容时，用最少的操作看懂它，需要时还能留住它。
</p>

<p align="center">
  <a href="README.md">English</a> &nbsp;|&nbsp; <b>简体中文</b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/平台-Windows%2010%2F11-0078D6?style=flat-square&logo=windows" alt="Windows 10/11" />
  <img src="https://img.shields.io/badge/Linux-规划中%20(KDE%20Wayland)-grey?style=flat-square&logo=linux" alt="Linux 规划中" />
  <img src="https://img.shields.io/badge/技术栈-Tauri%202%20%2B%20Rust%20%2B%20Svelte%205-2563EB?style=flat-square" alt="Tauri 2 + Rust + Svelte 5" />
  <img src="https://img.shields.io/badge/本地%20OCR-PP--OCRv5%20%2F%20ONNX-10B981?style=flat-square" alt="PaddleOCR PP-OCRv5" />
  <img src="https://img.shields.io/badge/安装包-%3C%2010%20MB-6366F1?style=flat-square" alt="轻量安装包" />
</p>

---

## 简介

SnapMind 把**截图、标注、贴图、离线 OCR、AI / 机器翻译、划词翻译、长截图和录屏**整合在一个轻量、顺手的原生桌面应用中，所有识别与翻译操作都归结为一条清晰的主线：

```text
截图 / 划词 / 剪贴板
  ├─► 本地 OCR（PP-OCRv5 / SLANet_plus）或 视觉大模型
  │     ├─► 纯文本（按段落 / 按行 / 框选局部文字）
  │     ├─► 结构化输出（Markdown · Excel/CSV 表格 · LaTeX 公式）
  │     └─► 翻译（可编辑卡片 · 截图原位覆盖 · 文字贴图）
  └─► 矢量标注 · 置顶贴图 · 滚动长截图 · 区域录屏（MP4 / GIF）
```

- **快且轻量**：多显示器预热冻结帧遮罩（按键到遮罩出现约 45 ms），安装包不到 10 MB；离线 OCR 模型按需下载。
- **隐私友好**：本地 OCR 基于 ONNX Runtime 完全离线运行；API Key 仅保存在系统凭据管理器中，不写入明文配置文件。
- **云母（Mica）视觉**：浅色 / 深夜双模式（支持跟随系统）、4 种强调色、跟手微动效，支持「减弱动效」。

---

## 核心功能

### 截图、吸附与标注
- **多屏高 DPI 冻结帧（`Shift+F1`）**：自动吸附窗口与界面控件（UI Automation），滚动滚轮可切换上级 / 下级元素。
- **像素放大镜与取色**：框选前后均显示光标坐标与放大像素；按 `P` 切换 `HEX` / `RGB` / `HSL`，按 `C` 复制色值，`W A S D` 逐像素微调光标。
- **完整标注工具**：矩形 / 椭圆、画笔 / 荧光笔、箭头 / 直线、可编辑文字、序号、基于真实像素的马赛克 / 模糊、橡皮擦与撤销；导出时按原始物理分辨率合成，高分屏同样锐利。
- **灵活保存与复制（`Ctrl+S` / `Ctrl+C`）**：内置可视化保存窗口（支持 PNG / JPG / WebP、实时体积预估、锁定比例缩放、自定义文件名模板与常用目录），也支持切换为系统另存为或一键直接保存；复制时同时写入位图、PNG 和文件对象（`CF_HDROP`），可直接粘贴到聊天软件或资源管理器。

### 离线本地 OCR 与结构化识别
- **本地 PP-OCRv5 + SLANet_plus**：基于 PaddlePaddle 官方开源模型（Apache-2.0，可商用）与 ONNX Runtime 本地推理；首次使用时按地区自动选源下载（中国大陆优先魔搭 ModelScope，海外优先 Hugging Face）并逐个校验 SHA-256。
- **智能段落与分栏还原**：自动按阅读顺序还原单栏 / 双栏段落，中日韩无缝拼接、拉丁文自动补空格并拆行尾连字符，支持识别代码块。
- **检测框局部挑字**：左键单击检测框即可挑选指定行（`Ctrl+A` 全选后再点可反选），也可以右键划过检测框只提取局部区域。
- **识别为 Markdown（`Shift+Alt+R`）**：支持编辑源码与 Obsidian 方言实时预览（表格、提示块、高亮、KaTeX 公式），可直接导出 `.md`。
- **表格识别**：支持 HTML 渲染预览与单元格直接编辑，可切换表格 / Markdown / CSV 视图；一键复制带格式表格直接粘入 Excel / Word（保留合并单元格），或直接导出 `.xlsx`。
- **公式识别 → LaTeX**：调用视觉模型提取公式，内置 KaTeX 实时预览与报错定位，一键复制 LaTeX 源码或 `$$...$$`。

### AI 翻译、原位覆盖与划词翻译
- **截图原位覆盖**：结合本地 OCR 坐标与背景/文字取色，将译文自适应字号直接覆盖在原图对应位置（自动跳过代码、纯数字与公式），可直接复制图片或贴图。
- **一步到位视觉模式**：由单个视觉大模型一次请求直接完成识图与翻译。
- **划词翻译（`Ctrl+Alt+D`）**：优先通过 UI Automation 无感取词，不支持时模拟复制并在取词后完整还原原剪贴板；结果以可拖动小卡片弹出在鼠标旁，点击图钉固定后，再次划词会在同一张卡片里无缝接着翻译。
- **细致翻译偏好**：双语对照、专业术语自动保留、不翻译词汇表，以及「原文已是目标语言时自动翻成第二语言」。

### 24+ 模型服务商、机器翻译与备用链
- **多协议流式接入**：原生支持 **OpenAI 兼容**、**Anthropic Claude**、**Google Gemini** 流式对话接口，并内置 **DeepL**、**Google Cloud Translation**、**微软 Translator** 官方机器翻译接口。
- **24 家内置预设**：DeepSeek、Anthropic、Google Gemini、OpenAI、OpenRouter、通义千问（百炼）、Kimi、智谱 GLM、火山方舟（豆包）、硅基流动、xAI、Mistral、MiniMax、阶跃星辰、百度千帆、腾讯混元、魔搭社区、Groq、Together AI、Fireworks、NVIDIA NIM、**Ollama（本地）**、**LM Studio（本地）**，以及任意自定义 OpenAI 兼容服务；支持国内站 / 国际站一键切换。
- **视觉模型自动识别**：按「服务商自报 → 内置 `models.dev` 能力目录 → 名称规则」三级精准识别视觉与推理模型。
- **有序备用链（Fallback）**：识别、翻译、一步到位各自独立配置有序链路；遇到超时、报错或空结果自动切换下一项，并在结果卡片与历史中标明实际使用的后端。

### 置顶贴图与历史记录
- 将截图（`Ctrl+T`）或剪贴板内容（`Shift+F3`，支持图片和自动排版文字卡片）钉在屏幕最前。
- **贴图自动 OCR**：图片贴出后后台自动识别，可直接在贴图上拖选复制文字。
- **丰富控制**：滚轮缩放（10%–800%）、`Ctrl+滚轮` 调不透明度、鼠标穿透（`Ctrl+Shift+X`）、贴图原位标注（`Space`）、防丢失悬浮工具条。
- **历史记录与崩溃恢复**：关闭贴图和完成翻译后自动写入本地 SQLite 历史库，支持全文搜索、重新贴出和按条数 / 空间自动清理；异常崩溃后重启可自动恢复此前打开的贴图。

### 长截图与录屏
- **滚动长截图（`L`）**：支持自动滚动与手动滚动，实时拼接预览，自动去除固定页头页尾并裁掉滚动条，支持纵向 / 横向拼接。
- **区域录屏（`R`）**：支持 MP4（Windows Media Foundation H.264 + WASAPI 系统内录 AAC）和 GIF，可选帧率、支持暂停/继续，停止后自动保存并将文件复制到剪贴板。

### 高度可定制与数据安全
- **自定义工具栏**：截图和贴图工具条均可拖拽排序、隐藏或收进「更多」菜单。
- **快捷键录制**：所有全局快捷键支持可视化录制与冲突检测。
- **可选悬浮球**：支持贴靠屏幕左右边缘、自动半隐藏的毛玻璃悬浮球。
- **配置导入导出与自动备份**：支持按模块导出/导入配置（勾选包含 API Key 时采用 PBKDF2-SHA256 + AES-256-GCM 加密），并支持变更前与每周自动备份。
- **应用内自动更新**：支持基于 GitHub Releases 的签名校验更新。

---

## 默认快捷键

所有全局快捷键均可在 **设置 → 快捷键** 中自定义。

### 全局快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Shift+F1` | 截图，进入编辑态 |
| `Shift+F3` | 贴剪贴板内容（图片贴图片，文字排版成文字卡片） |
| `Ctrl+Alt+D` | 划词翻译 |
| `Ctrl+Shift+O` | 截图 → 识别 → 翻译 |
| `Ctrl+Shift+S` | 截图 → 识别文字（自动复制并打开可编辑卡片） |

### 截图编辑态与贴图常用快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl+C` / `Enter` | 复制并结束 |
| `Ctrl+T` | 贴图到屏幕并结束 |
| `Ctrl+S` | 保存（保存窗口 / 系统另存为 / 直接保存） |
| `Space` | 进入 / 退出标注模式 |
| `1` – `7` | 切换标注工具 |
| `Ctrl+Z` | 撤销标注 |
| `Shift+C` | 识别文字（OCR） |
| `Shift+Alt+R` | 识别为 Markdown |
| `Ctrl+Q` | 翻译 |
| `L` / `R` | 长截图 / 录屏 |
| `P` / `C` | 切换色值格式（HEX/RGB/HSL）/ 复制颜色值 |
| `W A S D` | 逐像素移动光标 |
| `方向键` / `Shift+方向键` | 微调选区 1 px / 10 px |
| `H` | 显示 / 隐藏快捷键提示 |
| `Esc` | 逐级退出 |

---

## 下载与安装

### Windows（10 / 11 x64）
1. 前往仓库的 **Releases** 页面下载最新的 `SnapMind_<版本>_x64-setup.exe`。
2. 双击运行安装程序：支持「为所有用户安装（`C:\Program Files\SnapMind`）」和「只为当前用户安装（`%LOCALAPPDATA%\SnapMind`）」。
3. 首次启动会打开新手引导，可按需一键下载本地离线 OCR 模型（约 21 MB）。

> **Linux（KDE Plasma / Wayland）** 版本已在开发路线中（基于 `xdg-desktop-portal`、GlobalShortcuts portal 与 layer-shell）。

---

## 从源码构建

### 环境要求
- **Node.js** 22+ 与 **pnpm**
- **Rust** 稳定版工具链（`rustup`、`x86_64-pc-windows-msvc`）
- **Visual Studio Build Tools**（C++ 桌面开发工作负载 + Windows SDK）
- **WebView2 Runtime**（Windows 10/11 通常已内置）

### 开发与测试

```bash
# 安装前端依赖
pnpm install

# 启动开发模式
pnpm tauri dev

# 类型检查与单元测试
pnpm check
pnpm test
cargo test --workspace
```

### 打包安装包

```bash
# 打 NSIS 安装包（不自动安装到本机）
pnpm release --no-install

# 先递增版本号再打包（支持 patch | minor | major | 具体版本号如 1.0.0）
pnpm release patch --no-install
```

构建产物位于 `target/release/bundle/nsis/`。

---

## 目录结构

```text
SnapMind/
├─ src-tauri/                 # Tauri 2 应用外壳：窗口管理、命令、托盘、自动更新
├─ crates/
│  ├─ snapmind-core/          # 平台无关核心层：配置、提示词、历史记录、长图拼接、导出
│  ├─ snapmind-providers/     # 大模型（OpenAI / Claude / Gemini）与机器翻译适配器及备用链
│  ├─ snapmind-ocr/           # ONNX Runtime 本地推理（PP-OCRv5、SLANet_plus）、版面与段落引擎
│  └─ snapmind-platform/      # 平台抽象层与原生实现（截屏、UI Automation、剪贴板、录屏）
├─ ui/                        # Svelte 5 + TypeScript + Vite 多入口前端（遮罩、贴图、划词卡片、设置、悬浮球）
├─ scripts/                   # 模型下载、视觉模型索引构建、本地打包与 Release 发布脚本
└─ design/                    # 应用图标与可交互设计原型
```

---

## 开源组件与致谢

SnapMind 受益于以下优秀的开源项目：

- **[Tauri 2](https://tauri.app/)** 与 **[Svelte 5](https://svelte.dev/)**（MIT / Apache-2.0）— 跨平台桌面框架与响应式界面
- **[PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)**（`PP-OCRv5`、`SLANet_plus`，Apache-2.0）— 本地文字检测、识别与表格结构模型
- **[ONNX Runtime](https://onnxruntime.ai/)** / [`ort`](https://github.com/pykeio/ort)（MIT）— 本地神经网络推理引擎
- **[KaTeX](https://katex.org/)** 与 **[marked](https://marked.js.org/)**（MIT）— LaTeX 数学公式与 Markdown 渲染
- **[models.dev](https://models.dev/)**（MIT）— 模型能力目录数据

---

## 许可证

> 公开许可协议尚未最终确定（当前 `Cargo.toml` 为 `UNLICENSED`）。在公开发布前，建议在仓库根目录补充 `LICENSE` 文件（如 MIT / Apache-2.0 / GPL-3.0）并同步更新此处说明。
