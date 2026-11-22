// 生成 OCR 评测样本：写 HTML 页面，用本机 Edge 无头截图，真值（段落和行）由页面内容直接给出。
//
// 用法：node scripts/make-ocr-samples.mjs [样本名…]（不写名字就全部重新生成）
// 输出：crates/snapmind-ocr/tests/fixtures/ocr-samples/<名字>.png + <名字>.txt
//   .txt 格式：一行文字一行，段落之间空一行。
//
// 每一行都用 <br> 显式断开并禁止自动换行，所以截图里的行和真值一一对应。
// 内容覆盖：中英文文章、中英混排、界面小字、深色聊天、代码、双栏、日文、高分屏、低对比、标题列表、菜单，
// 以及首行缩进、无分隔线的右键菜单、悬挂缩进的编号列表、2 倍行距带连字符的正文。
import { execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'crates', 'snapmind-ocr', 'tests', 'fixtures', 'ocr-samples');
const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
/** 一个段落：若干行 */
const p = (...lines) => lines;

/**
 * 样本：name、页面尺寸、缩放、样式；blocks 是段落数组；layout 决定段落怎么摆（默认依次往下）。
 * 每个段落渲染成 <p class="b"> 行 <br> 行 </p>，额外的 class 用 cls 指定。
 */
const SAMPLES = [
  {
    name: 'article-en',
    css: 'body{font:17px/1.6 Georgia,serif;color:#222;background:#fff;padding:40px 56px}',
    blocks: [
      p('Most screenshots contain text that was never meant to be copied:', 'labels in a dialog, a slide in a meeting, a frame of a video.', 'Reading it back should take one keystroke, not five minutes.'),
      p('A good result keeps the paragraphs the way the author wrote them,', 'joins the lines inside each paragraph, and leaves code alone.'),
      p('When the text is in another language, the translation should', 'appear right next to the original, ready to copy or pin.'),
    ],
  },
  {
    name: 'article-zh',
    css: 'body{font:16px/1.75 "Microsoft YaHei UI",sans-serif;color:#1f1f1f;background:#fff;padding:40px 56px}',
    blocks: [
      p('截图里的文字大多不是为了复制而存在的：对话框里的标签、', '会议上的一页幻灯片、视频里的一帧画面。把它们读出来，', '应该只需要按一下快捷键，而不是花上五分钟。'),
      p('好的结果会保留作者写下的段落，把段落内部的行拼接起来，', '并且不去改动代码和表格。'),
      p('如果文字是外语，译文应该紧挨着原文出现，可以直接复制或贴到屏幕上。'),
    ],
  },
  {
    name: 'mixed-tech',
    css: 'body{font:15px/1.7 "Microsoft YaHei UI",sans-serif;color:#202124;background:#fafafa;padding:36px 48px}',
    blocks: [
      p('在 Windows 上，SnapMind 用 GDI 抓取整块屏幕，延迟约 35 ms。', '遮罩窗口常驻在屏幕外，按下 Shift+F1 后再移进来。'),
      p('翻译链支持 DeepSeek、Claude 和 Gemini，也可以加入 DeepL', '或微软翻译作为备用；超时默认 8 秒，失败时自动切换。'),
      p('OCR 使用 PP-OCRv5 mobile 模型，通过 ONNX Runtime 1.31 在 CPU 上运行。'),
    ],
  },
  {
    name: 'ui-settings',
    size: [1100, 640],
    css: `body{font:13px/1.5 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#1b1b1b;background:#f3f3f3;padding:28px 40px}
      .row{display:flex;justify-content:space-between;align-items:center;background:#fff;border-radius:8px;padding:12px 16px;margin:0 0 6px}
      .row .b{margin:0}.desc{color:#5f5f5f;font-size:12px}.btn{border:1px solid #ccc;border-radius:6px;padding:4px 12px;background:#fdfdfd}`,
    html: (b) => `<h2 class="b" style="font-size:20px;margin:0 0 14px">${b[0].html}</h2>` +
      [1, 3, 5, 7]
        .map((i) => `<div class="row"><div><div class="b">${b[i].html}</div><div class="b desc">${b[i + 1].html}</div></div><div class="btn">${['更改', '浏览…', '恢复默认', 'Edit'][(i - 1) / 2]}</div></div>`)
        .join(''),
    blocks: [
      p('截图设置'),
      p('截图时显示操作提示'), p('截图时按 H 也可以切换，会记住'),
      p('文件名模板'), p('Supports yyyy MM dd HH mm ss SSS'),
      p('Ctrl+S 的行为'), p('保存窗口可以改名、换格式和尺寸'),
      p('Default save folder'), p('Pictures\\SnapMind'),
    ],
    // 按钮文字也在图里：作为单独的段落计入真值
    extra: [p('更改'), p('浏览…'), p('恢复默认'), p('Edit')],
  },
  {
    name: 'chat-dark',
    css: `body{font:14px/1.55 "Microsoft YaHei UI",sans-serif;color:#e8eaed;background:#1e1f22;padding:28px 40px}
      .b{max-width:none;padding:10px 14px;border-radius:12px;margin:0 0 12px;width:fit-content}
      .me{background:#3b5bdb;margin-left:auto}.them{background:#2b2d31}`,
    blocks: [
      p('明天的评审几点开始？'),
      p('下午两点，会议室换到了 4 楼。', 'Slides are in the shared folder.'),
      p('收到，我把截图翻译的演示也准备一下。'),
      p('Great, keep it under five minutes.'),
    ],
    blockClass: (i) => (i % 2 === 0 ? 'me' : 'them'),
  },
  {
    name: 'code',
    css: `body{font:14px/1.6 Consolas,monospace;color:#1f2328;background:#f6f8fa;padding:32px 40px}
      .b{white-space:pre}`,
    blocks: [
      p('fn main() {', '    let text = ocr.recognize(image)?;', '    println!("{} lines", text.len());', '}'),
      p('// Run: cargo run --release -p snapmind-ocr'),
    ],
  },
  {
    name: 'two-column',
    size: [1280, 720],
    css: `body{font:12.5px/1.55 "Times New Roman","SimSun",serif;color:#111;background:#fff;padding:36px 48px;columns:2;column-gap:48px}
      h1{column-span:all;font-size:20px;margin:0 0 16px}`,
    html: (b) => `<h1 class="b">${b[0].html}</h1>` + b.slice(1).map((x) => `<p class="b">${x.html}</p>`).join(''),
    blocks: [
      p('Paragraph Grouping for Screen Text'),
      p('Screen text differs from scanned pages: fonts are crisp,', 'lines are short, and layouts mix menus with prose.', 'We group detected lines by their left edges and gaps.'),
      p('Lines whose vertical gap is below 0.6 of the line height', 'and whose left edges align are merged into one paragraph.'),
      p('A first-line indent, a sudden change of spacing, or a', 'different font size starts a new paragraph instead.'),
      p('Chinese and Japanese lines are joined directly, while', 'Latin lines get a space and lose trailing hyphens.'),
    ],
  },
  {
    name: 'japanese',
    css: 'body{font:16px/1.8 "Yu Gothic UI","Meiryo",sans-serif;color:#222;background:#fff;padding:40px 56px}',
    blocks: [
      p('設定を保存しました。次回の起動時に反映されます。'),
      p('スクリーンショットの文字を選択して、', 'そのままコピーできます。'),
      p('翻訳結果はカードに表示され、編集もできます。'),
    ],
  },
  {
    name: 'article-zh-hidpi',
    scale: 1.5,
    size: [960, 560],
    css: 'body{font:15px/1.7 "Microsoft YaHei UI",sans-serif;color:#262626;background:#fff;padding:32px 40px}',
    blocks: [
      p('高分屏上的截图像素更多，文字更清晰，', '但同样大小的区域里，识别要处理的像素也更多。'),
      p('检测模型的输入长边限制在 1440 像素，超出时等比缩小。'),
    ],
  },
  {
    name: 'low-contrast',
    css: 'body{font:14px/1.6 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#9a9a9a;background:#f1f1f1;padding:36px 48px}',
    blocks: [
      p('Placeholder text is often light gray on a light background.'),
      p('浅灰色的提示文字对比度低，也要能认出来。', 'Last saved 3 minutes ago'),
    ],
  },
  {
    name: 'headings-list',
    css: `body{font:15px/1.6 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#1f1f1f;background:#fff;padding:36px 52px}
      .h1{font-size:26px;font-weight:600;margin:0 0 10px}.h2{font-size:19px;font-weight:600;margin:18px 0 8px}.li{margin:0 0 4px}`,
    blocks: [
      p('快捷键'),
      p('截图与贴图'),
      p('• Shift+F1 截图，进入编辑态'),
      p('• Shift+F3 贴剪贴板内容'),
      p('• Ctrl+Shift+X 切换贴图的鼠标穿透'),
      p('Translation'),
      p('• Ctrl+Q translate the selection'),
      p('• Ctrl+Alt+D translate selected text'),
    ],
    blockClass: (i) => (i === 0 ? 'h1' : i === 1 || i === 5 ? 'h2' : 'li'),
  },
  {
    name: 'tiny-menu',
    size: [640, 420],
    css: `body{font:11px/1.4 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#202020;background:#fff;padding:20px 28px}
      .b{padding:5px 8px;margin:0;border-bottom:1px solid #eee}`,
    blocks: [p('新建窗口'), p('打开最近的文件'), p('Save as PDF…'), p('打印预览'), p('Preferences'), p('关于 SnapMind')],
  },
  // —— M4.3 加的段落样本 ——
  {
    // 中文排版常见的首行缩进、段落之间不留空
    name: 'indent-zh',
    css: `body{font:15px/1.8 "Microsoft YaHei UI",sans-serif;color:#222;background:#fff;padding:40px 56px}
      .b{margin:0;text-indent:2em}`,
    blocks: [
      p('截图之后按下识别，文字会按段落排好，', '段落内部的换行被去掉，可以直接粘贴到文档里。'),
      p('如果原文是首行缩进的排版，段落之间没有空行，', '就靠缩进来判断哪里是新段落的开始。'),
      p('最后一段只有一行。'),
    ],
  },
  {
    // Windows 11 风格的右键菜单：没有分隔线，右侧是快捷键
    name: 'context-menu',
    size: [640, 420],
    css: `body{font:12px/1.4 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#1a1a1a;background:#f9f9f9;padding:24px 32px}
      .m{width:300px}.it{display:flex;justify-content:space-between;padding:8px 12px}.k{color:#6b6b6b}`,
    html: (b) => `<div class="m">${[0, 2, 4, 6, 8]
      .map((i) => `<div class="it"><span class="b">${b[i].html}</span><span class="b k">${b[i + 1].html}</span></div>`)
      .join('')}</div>`,
    blocks: [
      p('剪切'), p('Ctrl+X'),
      p('复制'), p('Ctrl+C'),
      p('粘贴为纯文本'), p('Ctrl+Shift+V'),
      p('Select all'), p('Ctrl+A'),
      p('Rename'), p('F2'),
    ],
  },
  {
    // 编号列表，每一项换行后和文字对齐（悬挂缩进）
    name: 'list-wrap',
    css: `body{font:15px/1.6 "Segoe UI","Microsoft YaHei UI",sans-serif;color:#1f1f1f;background:#fff;padding:36px 52px}
      .b{margin:0 0 6px}.li{padding-left:22px;text-indent:-22px}`,
    blocks: [
      p('Getting started'),
      p('1. Press Shift+F1 to start a capture, then drag', 'to select the region you want to keep.'),
      p('2. Press Ctrl+Shift+S to recognize the text in', 'the region and copy it as paragraphs.'),
      p('3. 按 Ctrl+Q 翻译选中的区域，译文会出现在', '选区旁边的卡片里。'),
    ],
    blockClass: (i) => (i === 0 ? '' : 'li'),
  },
  {
    // 行距 2 倍的英文正文、标题和正文之间间距很小、行尾连字符
    name: 'article-loose',
    css: `body{font:15px/2 "Segoe UI",sans-serif;color:#2b2b2b;background:#fff;padding:36px 52px}
      .h{font-size:22px;font-weight:600;margin:0 0 4px;line-height:1.3}.b{margin:0 0 20px}`,
    blocks: [
      p('Reading text back from pixels'),
      p('Detected lines are grouped into paragraphs by their gaps, align-', 'ment and font size, so that copied text keeps its structure.'),
      p('Loose line spacing is common on modern web pages, and it should', 'not be mistaken for the gap between two paragraphs.'),
    ],
    blockClass: (i) => (i === 0 ? 'h' : ''),
  },
];

function page(s) {
  const blocks = s.blocks.map((lines, i) => {
    const cls = s.blockClass?.(i) ?? '';
    return { cls, html: lines.map(esc).join('<br>') };
  });
  // 自定义布局（html）拿到 { cls, html } 自己包标签；默认每段一个 <p>
  const body = s.html ? s.html(blocks) : blocks.map((b) => `<p class="b ${b.cls}">${b.html}</p>`).join('\n');
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    *{box-sizing:border-box}html,body{margin:0}.b{white-space:nowrap;margin:0 0 18px}
    ${s.css}</style></head><body>${body}</body></html>`;
}

function main() {
  mkdirSync(OUT, { recursive: true });
  const work = join(tmpdir(), 'snapmind-ocr-samples');
  rmSync(work, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });
  const only = process.argv.slice(2);
  const todo = only.length ? SAMPLES.filter((s) => only.includes(s.name)) : SAMPLES;
  for (const s of todo) {
    const [w, h] = s.size ?? [1200, 640];
    const html = join(work, `${s.name}.html`);
    writeFileSync(html, page(s), 'utf8');
    const png = join(OUT, `${s.name}.png`);
    execFileSync(EDGE, [
      '--headless=new',
      '--disable-gpu',
      '--hide-scrollbars',
      '--no-first-run',
      `--user-data-dir=${join(work, 'profile')}`,
      `--force-device-scale-factor=${s.scale ?? 1}`,
      `--window-size=${w},${h}`,
      `--screenshot=${png}`,
      pathToFileURL(html).href,
    ], { stdio: 'ignore' });
    const truth = [...s.blocks, ...(s.extra ?? [])].map((lines) => lines.join('\n')).join('\n\n');
    writeFileSync(join(OUT, `${s.name}.txt`), `${truth}\n`, 'utf8');
    console.log(`  ✓ ${s.name}（${w}×${h} @${s.scale ?? 1}x）`);
  }
  console.log(`${todo.length} 个样本 → ${OUT}`);
}

main();
