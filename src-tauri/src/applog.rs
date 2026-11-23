//! 应用日志（M11.2）：写进 `配置目录/logs/snapmind.log`（滚动，见 `snapmind_core::applog`）。
//!
//! 发行版没有控制台，以前的 eprintln 都看不到；现在全部写文件，调试版同时打到 stderr。
//! 崩溃（panic）时把位置、内容和调用栈写进日志再交给默认处理。

use snapmind_core::i18n::tr;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use snapmind_core::applog::{Log, KEEP, MAX_BYTES};
use snapmind_core::export::now_local;

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();

/// 打开日志文件并装上 panic 钩子。打不开时只用 stderr。
pub fn init(dir: PathBuf) {
    match Log::open(&dir, MAX_BYTES, KEEP) {
        Ok(log) => {
            let _ = LOG.set(Mutex::new(log));
        }
        Err(e) => eprintln!("打开日志文件失败（{}）：{e}", dir.display()),
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let bt = std::backtrace::Backtrace::force_capture();
        write("panic", &format!("线程 {} 崩溃：{info}\n{bt}", thread.name().unwrap_or("?")));
        default(info);
    }));
    write(
        "info",
        &format!("SnapMind {} 启动（{}）", snapmind_core::version(), snapmind_platform::platform_name()),
    );
}

/// 日志目录（「关于」页的「打开日志文件夹」用）。
pub fn dir() -> Option<PathBuf> {
    LOG.get().map(|l| l.lock().unwrap_or_else(|e| e.into_inner()).dir().to_path_buf())
}

pub fn write(level: &str, msg: &str) {
    if cfg!(debug_assertions) || LOG.get().is_none() {
        eprintln!("[{level}] {msg}");
    }
    if let Some(log) = LOG.get() {
        // panic 钩子里也会调用：锁被毒化时照样写
        let mut log = log.lock().unwrap_or_else(|e| e.into_inner());
        let _ = log.write(level, msg, now_local());
    }
}

/// 记一条警告（操作失败但不影响继续使用）。用法同 `format!`。
#[macro_export]
macro_rules! log_warn {
    ($($t:tt)*) => { $crate::applog::write("warn", &format!($($t)*)) };
}

/// 记一条普通信息。
#[macro_export]
macro_rules! log_info {
    ($($t:tt)*) => { $crate::applog::write("info", &format!($($t)*)) };
}

/// 打开日志文件夹。
#[tauri::command]
pub fn log_open() -> Result<(), String> {
    let dir = dir().ok_or_else(|| tr("日志文件没有打开"))?;
    crate::export::open_folder(dir.to_string_lossy().into_owned())
}
