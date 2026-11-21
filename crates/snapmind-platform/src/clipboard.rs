//! 剪贴板。M1.4 先支持文字；图片和「粘贴成文件」在 M1.6 加入。

#[derive(Debug, thiserror::Error)]
#[error("写入剪贴板失败：{0}")]
pub struct ClipboardError(String);

/// 写入纯文字。
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    let mut cb = arboard::Clipboard::new().map_err(|e| ClipboardError(e.to_string()))?;
    cb.set_text(text).map_err(|e| ClipboardError(e.to_string()))
}
