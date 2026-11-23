<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" height="96" alt="SnapMind Logo" />
</p>

<h1 align="center">SnapMind</h1>

<p align="center">
  <strong>AI-powered screenshot, OCR, translation, annotation, pin &amp; screen recording tool for desktop.</strong><br />
  Understand foreign text on your screen with minimal clicks — and keep it right where you need it.
</p>

<p align="center">
  <b>English</b> &nbsp;|&nbsp; <a href="README_zh.md">简体中文</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2F11-0078D6?style=flat-square&logo=windows" alt="Windows 10/11" />
  <img src="https://img.shields.io/badge/Linux-Planned%20(KDE%20Wayland)-grey?style=flat-square&logo=linux" alt="Linux Planned" />
  <img src="https://img.shields.io/badge/Built%20with-Tauri%202%20%2B%20Rust%20%2B%20Svelte%205-2563EB?style=flat-square" alt="Tauri 2 + Rust + Svelte 5" />
  <img src="https://img.shields.io/badge/Local%20OCR-PP--OCRv5%20%2F%20ONNX-10B981?style=flat-square" alt="PaddleOCR PP-OCRv5" />
  <img src="https://img.shields.io/badge/Installer-%3C%2010%20MB-6366F1?style=flat-square" alt="Lightweight Installer" />
</p>

---

## Overview

SnapMind unifies screen capture, offline OCR, AI & machine translation, visual annotation, floating pin windows, scrolling screenshots, and screen recording into a single fast, native desktop application:

```text
Screenshot / Selection / Clipboard
  ├─► Local OCR (PP-OCRv5 / SLANet_plus) or Vision LLM
  │     ├─► Plain Text (paragraph / line / partial box selection)
  │     ├─► Structured Output (Markdown · Excel/CSV Table · LaTeX Formula)
  │     └─► Translation (Editable Card · In-Place Image Overlay · Pin Card)
  └─► Annotate · Pin on Screen · Scroll Stitch · Record MP4 / GIF
```

- **Instant & Lightweight** — Pre-warmed multi-monitor freeze-frame overlay (~45 ms response), < 10 MB installer; offline OCR models are downloaded on demand.
- **Privacy First** — Local OCR runs completely offline via ONNX Runtime; API keys are stored strictly in the OS Credential Manager (Keyring) and never written to plain-text config files.
- **Mica Visual Design** — Clean Light & Night themes (Follow System) with 4 accent colors, crisp micro-animations, and Reduced Motion support.

---

## Key Features

### Smart Screenshot & Precision Tools
- **Multi-Monitor & Mixed-DPI Freeze Frame (`Shift+F1`)**: Instant desktop capture with automatic window and UI element snapping via UI Automation (use the scroll wheel to step up/down the element hierarchy).
- **Pixel Magnifier & Color Picker**: Real-time zoomed preview with cursor coordinates; press `P` to cycle `HEX` / `RGB` / `HSL`, `C` to copy color, and `W A S D` for 1 px cursor nudging.
- **Full Annotation Suite**: Rectangle/Ellipse, Brush/Highlighter, Arrow/Line, Editable Text, Numbered Steps, Pixel-Accurate Mosaic/Blur, Eraser, and Undo — composited at full physical DPI on export.
- **Flexible Export (`Ctrl+S`)**: Visual save dialog (PNG / JPG / WebP with live file-size estimate, resize, custom filename templates, and favorite folders), system Save As, or one-key direct save. Copying (`Ctrl+C` / `Enter`) writes DIB, PNG, and file drop (`CF_HDROP`) simultaneously so you can paste into chat apps, editors, or File Explorer seamlessly.

### Offline Local OCR & Structured Recognition
- **Local PP-OCRv5 + SLANet_plus**: Powered by ONNX Runtime with Apache-2.0 licensed models (commercial-use friendly). Automatically selects the fastest official mirror by region (ModelScope in Mainland China, Hugging Face globally) with SHA-256 verification.
- **Smart Paragraph Reconstruction**: Rebuilds reading order across single- and multi-column layouts, joins CJK cleanly, preserves Latin spacing/de-hyphenation, and detects code blocks.
- **Interactive Box Selection**: All detected boxes start selected; left-click a box to select or deselect it, `Ctrl+A` selects all or none. The card shows only the selected text.
- **Recognize as Markdown (`Shift+Alt+R`)**: Editable source and live Obsidian-flavored preview (tables, callouts, highlights, KaTeX math) with `.md` export.
- **Table Recognition**: Rendered HTML preview with inline cell editing, Markdown & CSV views, rich HTML + TSV copy (paste directly into Excel/Word with merged cells intact), and `.xlsx` export.
- **Formula → LaTeX**: Vision-model extraction with real-time KaTeX preview, error diagnostics, and one-click `$$...$$` copy.

### AI Translation, In-Place Overlay & Selection Translate
- **In-Place Image Overlay**: Renders translated text directly over the original screenshot while matching background/foreground colors and font sizes (automatically skipping code, numbers, and formulas).
- **One-Shot Vision Mode**: Let a single vision-capable LLM read and translate an image in one call.
- **Selection Translation (`Ctrl+Alt+D`)**: Reads selected text via UI Automation (with clipboard-preserving fallback) into a draggable popup card beside your cursor. Pin the card to keep it on screen and reuse it for subsequent lookups.
- **Translation Preferences**: Bilingual output, custom glossary / do-not-translate list, automatic technical-term preservation, and smart fallback to a second language when source text already matches your target language.

### 24+ AI Providers, Machine Translation & Fallback Chains
- **Multi-Protocol Streaming**: Native adapters for **OpenAI-compatible**, **Anthropic Claude**, and **Google Gemini** streaming APIs, plus **DeepL**, **Google Cloud Translation**, and **Microsoft Translator**.
- **24 Built-in Presets**: DeepSeek, Anthropic, Google Gemini, OpenAI, OpenRouter, Qwen (DashScope), Kimi, Zhipu GLM, Doubao (Volcengine), SiliconFlow, xAI, Mistral, MiniMax, StepFun, Qianfan, Hunyuan, ModelScope, Groq, Together AI, Fireworks, NVIDIA NIM, **Ollama (local)**, **LM Studio (local)**, and arbitrary Custom OpenAI-compatible endpoints.
- **Smart Vision Detection**: Identifies vision-capable models automatically via provider metadata, bundled `models.dev` index, and naming rules.
- **Ordered Fallback Chains**: Configure independent chains for Recognition, Translation, and One-Shot. Automatically fails over on timeout, error, or empty response and shows which backend served the result.

### Always-on-Top Pin Windows
- Pin any capture (`Ctrl+T`) or clipboard image/text (`Shift+F3`) as a frameless floating window.
- **Auto Background OCR**: Directly drag-select and copy text from any pinned image.
- **Rich Controls**: Mouse-wheel zoom (10%–800%), `Ctrl+Scroll` opacity, click-through (`Ctrl+Shift+X`), in-place annotation mode (`Space`), and a hover toolbar with anti-dismiss delay.
- **History & Crash Recovery**: Closed pins and translations are saved in a local SQLite database with search, re-pinning, and configurable retention limits; open pins automatically recover after unexpected crashes.

### Scrolling Screenshot & Screen Recording
- **Scrolling Capture (`L`)**: Auto-scroll or manual scroll with real-time stitching preview, fixed header/footer deduplication, scrollbar trimming, and vertical/horizontal modes.
- **Screen Recording (`R`)**: Native MP4 (H.264 + AAC via Windows Media Foundation & WASAPI system audio loopback) and GIF recording, configurable FPS, pause/resume, and automatic file copy to clipboard when stopped.

### Customization & Data Management
- Customizable capture & pin toolbars (drag to reorder, hide, or move into the overflow menu).
- Custom global hotkeys with live conflict detection.
- Optional edge-docking floating ball.
- Selective configuration export/import (with optional PBKDF2-SHA256 + AES-256-GCM encrypted API keys) and automatic scheduled/pre-change backups.
- Signed automatic updates via GitHub Releases.

---

## Default Shortcuts

All global shortcuts can be customized in **Settings → Shortcuts**.

### Global Shortcuts

| Shortcut | Action |
| --- | --- |
| `Shift+F1` | Capture screenshot (enter edit overlay) |
| `Shift+F3` | Pin clipboard content (image or formatted text card) |
| `Ctrl+Alt+D` | Selection translate |
| `Ctrl+Shift+O` | Capture → OCR → Translate |
| `Ctrl+Shift+S` | Capture → OCR (copy text & open editable card) |

### Capture Overlay & Pin Shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+C` / `Enter` | Copy and finish |
| `Ctrl+T` | Pin to screen and finish |
| `Ctrl+S` | Save (dialog / System Save As / direct save) |
| `Space` | Enter / exit annotation mode |
| `1` – `7` | Select annotation tool |
| `Ctrl+Z` | Undo annotation |
| `Shift+C` | Recognize text (OCR) |
| `Shift+Alt+R` | Recognize as Markdown |
| `Ctrl+Q` | Translate |
| `L` / `R` | Scrolling capture / Screen recording |
| `P` / `C` | Cycle color format (HEX/RGB/HSL) / Copy pixel color |
| `W A S D` | Nudge cursor by 1 px |
| `Arrow` / `Shift+Arrow` | Adjust selection by 1 px / 10 px |
| `H` | Toggle shortcut hints |
| `Esc` | Step-by-step dismiss |

---

## Installation

### Windows (10 / 11 x64)
1. Download the latest `SnapMind_<version>_x64-setup.exe` from the **Releases** page.
2. Run the installer — supports both per-machine (`C:\Program Files\SnapMind`) and per-user (`%LOCALAPPDATA%\SnapMind`) installation.
3. On first launch, follow the quick welcome guide to optionally download the offline OCR models (~21 MB).

> **Linux (KDE Plasma / Wayland)** support is planned on the roadmap (via `xdg-desktop-portal`, GlobalShortcuts portal, and layer-shell).

---

## Building from Source

### Prerequisites
- **Node.js** 22+ and **pnpm**
- **Rust** stable toolchain (`rustup`, `x86_64-pc-windows-msvc`)
- **Visual Studio Build Tools** (C++ desktop workload + Windows SDK)
- **WebView2 Runtime** (pre-installed on Windows 10/11)

### Development & Testing

```bash
# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Run type checks and unit tests
pnpm check
pnpm test
cargo test --workspace
```

### Packaging

```bash
# Build NSIS installer (optionally bump version: patch | minor | major | x.y.z)
pnpm release --no-install
pnpm release patch --no-install
```

Build artifacts are generated in `target/release/bundle/nsis/`.

---

## Project Structure

```text
SnapMind/
├─ src-tauri/                 # Tauri 2 shell: window manager, commands, tray, updater
├─ crates/
│  ├─ snapmind-core/          # Platform-agnostic core: config, prompts, history, stitching, export
│  ├─ snapmind-providers/     # LLM (OpenAI / Claude / Gemini) & MT (DeepL / Google / MS) adapters + fallback chains
│  ├─ snapmind-ocr/           # ONNX Runtime inference (PP-OCRv5, SLANet_plus), layout & paragraph engine
│  └─ snapmind-platform/      # OS abstractions & native implementations (capture, UIA, clipboard, recording)
├─ ui/                        # Multi-entry Svelte 5 + TypeScript + Vite frontend (overlay, pin, selcard, settings, ball)
├─ scripts/                   # Model downloader, vision index generator, release & publish scripts
└─ design/                    # App icons and interactive design prototypes
```

---

## Open-Source Components & Acknowledgments

SnapMind is built on top of great open-source projects:

- **[Tauri 2](https://tauri.app/)** & **[Svelte 5](https://svelte.dev/)** (MIT / Apache-2.0) — Desktop runtime & reactive UI
- **[PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)** (`PP-OCRv5`, `SLANet_plus`, Apache-2.0) — Offline text detection, recognition, and table structure models
- **[ONNX Runtime](https://onnxruntime.ai/)** / [`ort`](https://github.com/pykeio/ort) (MIT) — Local model inference engine
- **[KaTeX](https://katex.org/)** & **[marked](https://marked.js.org/)** (MIT) — LaTeX math and Markdown rendering
- **[models.dev](https://models.dev/)** (MIT) — Model capability catalog

---

## License

> License not yet finalized (`UNLICENSED` in `Cargo.toml`). Add a `LICENSE` file (e.g., MIT / Apache-2.0 / GPL-3.0) before public distribution.
