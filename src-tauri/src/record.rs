//! 录屏（需求 3.10、M9.2）。
//!
//! 开始录制后遮罩进入「透出真实画面」模式（见 [`crate::live`]），三条后台线程：
//! - 采集：按帧率抓选区（带鼠标指针），打上「录制时间」（不含暂停），交给编码线程；
//!   编码跟不上时丢帧而不是越积越多（时间戳是真实的，丢帧只是少几帧，不会变快变慢）；
//! - 编码：MP4 交给 Media Foundation（H.264），GIF 交给 [`GifWriter`]；
//! - 声音（MP4 且勾选了录制声音）：WASAPI loopback 录下扬声器里的声音，转成 16 位立体声写进 AAC 音轨。
//!   没有声音在放时 loopback 什么都不给，这时按时间补静音，音画始终对齐；暂停期间的声音丢掉。
//!
//! 停止时等三条线程收尾、写好文件尾，把文件放进剪贴板（可以直接粘贴成文件）。放弃则删掉文件。

use snapmind_core::i18n::{tr, trf};
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Deserialize;
use snapmind_core::config::RecordFormat;
use snapmind_core::export;
use snapmind_core::gifenc::GifWriter;
use snapmind_platform::record::{aac_rate, to_stereo_i16, Loopback, Mp4Writer};
use snapmind_platform::{capture, clipboard, Rect};
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};

use crate::export::Region;
use crate::live::{self, Live};
use crate::overlay;
use crate::state::AppState;

#[derive(Default)]
pub struct RecordState {
    inner: Mutex<Option<Recording>>,
}

struct Recording {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
    path: PathBuf,
    /// MP4 写入器：编码和声音线程共用，停止时在这里写文件尾
    mp4: Option<Arc<Mutex<Option<Mp4Writer>>>>,
}

struct Shared {
    stop: AtomicBool,
    clock: Mutex<Clock>,
    /// 编码或声音线程出的错（停止时报告）
    error: Mutex<Option<String>>,
}

/// 录制时间：从开始算起，扣掉暂停的时间。
struct Clock {
    start: Instant,
    paused: Duration,
    since: Option<Instant>,
}

impl Clock {
    fn now(&self) -> Duration {
        let end = self.since.unwrap_or_else(Instant::now);
        end.duration_since(self.start).saturating_sub(self.paused)
    }
    fn is_paused(&self) -> bool {
        self.since.is_some()
    }
    fn set_paused(&mut self, on: bool) {
        match (on, self.since) {
            (true, None) => self.since = Some(Instant::now()),
            (false, Some(s)) => {
                self.paused += s.elapsed();
                self.since = None;
            }
            _ => {}
        }
    }
}

impl Shared {
    fn now(&self) -> Duration {
        self.clock.lock().expect("录屏时钟锁").now()
    }
    fn paused(&self) -> bool {
        self.clock.lock().expect("录屏时钟锁").is_paused()
    }
    fn fail(&self, e: impl std::fmt::Display) {
        let mut err = self.error.lock().expect("录屏错误锁");
        if err.is_none() {
            *err = Some(e.to_string());
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Options {
    pub format: RecordFormat,
    pub fps: u32,
    pub audio: bool,
}

/// H.264 要求宽高是偶数：多出来的一列 / 一行去掉。
pub fn even(region: Region) -> Region {
    Region { width: region.width & !1, height: region.height & !1, ..region }
}

/// 实际用的帧率：1–60，GIF 最多 30（浏览器里 GIF 每帧至少 20 ms）。
pub fn effective_fps(format: RecordFormat, fps: u32) -> u32 {
    let max = if format == RecordFormat::Gif { 30 } else { 60 };
    fps.clamp(1, max)
}

/// 开始录制。返回要保存到的文件路径。
#[tauri::command]
pub fn record_start(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    index: usize,
    region: Region,
    options: Options,
) -> Result<String, String> {
    let mon = overlay::monitor(&app, index).ok_or_else(|| tr("截图已结束"))?;
    let region = even(region);
    if region.width < 16 || region.height < 16 {
        return Err(tr("选区太小"));
    }
    stop_and_keep(&app); // 不会同时有两段录制
    let fps = effective_fps(options.format, options.fps);
    let config = state.config.get();
    let ext = if options.format == RecordFormat::Gif { "gif" } else { "mp4" };
    let dir = crate::export::default_dir(&app, &config);
    std::fs::create_dir_all(&dir).map_err(|e| trf("建不了保存目录：{e}", &[("e", &e.to_string())]))?;
    let stem = export::render_filename(&config.save.filename_template, export::now_local());
    let path = export::unique_path(&dir, &stem, ext);

    let shared = Arc::new(Shared {
        stop: AtomicBool::new(false),
        clock: Mutex::new(Clock { start: Instant::now(), paused: Duration::ZERO, since: None }),
        error: Mutex::new(None),
    });
    let rect = Rect { x: mon.x + region.x as i32, y: mon.y + region.y as i32, width: region.width, height: region.height };
    let interval = Duration::from_secs_f64(1.0 / fps as f64);

    // 编码器先建好：失败就直接报错，不进入录制
    let loopback = if options.format == RecordFormat::Mp4 && options.audio {
        match Loopback::open() {
            Ok(l) => Some(l),
            Err(e) => {
                crate::log_warn!("录屏：打不开系统声音，只录画面：{e}");
                None
            }
        }
    } else {
        None
    };
    enum Enc {
        Mp4(Arc<Mutex<Option<Mp4Writer>>>),
        Gif(GifWriter<BufWriter<std::fs::File>>),
    }
    let enc = match options.format {
        RecordFormat::Mp4 => {
            let rate = loopback.as_ref().map(|l| aac_rate(l.rate()));
            let w = Mp4Writer::create(&path, region.width, region.height, fps, rate).map_err(|e| e.to_string())?;
            Enc::Mp4(Arc::new(Mutex::new(Some(w))))
        }
        RecordFormat::Gif => {
            let f = std::fs::File::create(&path).map_err(|e| trf("建不了文件：{e}", &[("e", &e.to_string())]))?;
            Enc::Gif(GifWriter::new(BufWriter::new(f), region.width, region.height).map_err(|e| e.to_string())?)
        }
    };
    let mp4 = match &enc {
        Enc::Mp4(m) => Some(m.clone()),
        Enc::Gif(_) => None,
    };

    let live = Live::enter(&app, &window, (mon.x, mon.y));
    // 编码器和声音设备准备好了才开始计时：第一帧从 0 开始
    shared.clock.lock().expect("录屏时钟锁").start = Instant::now();
    let (tx, rx) = sync_channel::<(Vec<u8>, Duration)>(((fps as usize) / 2).max(4));
    let mut threads = Vec::new();

    // 采集
    {
        let shared = shared.clone();
        let live = live.clone();
        threads.push(spawn("record-capture", move || {
            let mut next = Instant::now();
            loop {
                if shared.stop.load(Ordering::SeqCst) || !live.poll() {
                    break;
                }
                if !shared.paused() {
                    match capture::capture_rect_with_cursor(rect) {
                        Ok(f) => {
                            let t = shared.now();
                            if let Err(TrySendError::Disconnected(_)) = tx.try_send((f.pixels().to_vec(), t)) {
                                break;
                            }
                        }
                        Err(e) => crate::log_warn!("录屏抓帧失败：{e}"),
                    }
                }
                next += interval;
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                } else {
                    next = now; // 落后了就不追（丢帧）
                }
            }
        }));
    }

    // 编码
    {
        let shared = shared.clone();
        threads.push(spawn("record-encode", move || match enc {
            Enc::Mp4(m) => {
                for (px, t) in rx {
                    if let Some(w) = m.lock().expect("MP4 锁").as_mut() {
                        if let Err(e) = w.video(&px, t, interval) {
                            shared.fail(e);
                        }
                    }
                }
            }
            Enc::Gif(mut g) => {
                for (px, t) in rx {
                    if let Err(e) = g.push(&px, t) {
                        shared.fail(e);
                    }
                }
                if let Err(e) = g.finish(shared.now()) {
                    shared.fail(e);
                }
            }
        }));
    }

    // 声音
    if let (Some(mut lb), Some(m)) = (loopback, mp4.clone()) {
        let shared = shared.clone();
        threads.push(spawn("record-audio", move || {
            let (from, ch) = (lb.rate(), lb.channels());
            let to = aac_rate(from);
            let mut pos = 0.0;
            let mut written: u64 = 0; // 已写的帧数（每帧一对左右声道）
            let mut pcm: Vec<i16> = Vec::new();
            while !shared.stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
                let samples = match lb.read() {
                    Ok(s) => s,
                    Err(e) => {
                        shared.fail(e);
                        break;
                    }
                };
                if shared.paused() {
                    continue; // 暂停时的声音丢掉
                }
                pcm.clear();
                to_stereo_i16(samples, ch, from, to, &mut pos, &mut pcm);
                // 没声音在放时 loopback 不给数据：落后超过 100 ms 就补静音
                let expected = (shared.now().as_secs_f64() * to as f64) as u64;
                let have = written + (pcm.len() / 2) as u64;
                if have + (to as u64 / 10) < expected {
                    let pad = (expected - have) as usize;
                    let mut silent = vec![0i16; pad * 2];
                    silent.extend_from_slice(&pcm);
                    pcm = silent;
                }
                if pcm.is_empty() {
                    continue;
                }
                let t = Duration::from_secs_f64(written as f64 / to as f64);
                if let Some(w) = m.lock().expect("MP4 锁").as_mut() {
                    if let Err(e) = w.audio(&pcm, t) {
                        shared.fail(e);
                    }
                }
                written += (pcm.len() / 2) as u64;
            }
        }));
    }

    *app.state::<RecordState>().inner.lock().expect("录屏锁") = Some(Recording { shared, threads, path: path.clone(), mp4 });
    Ok(path.to_string_lossy().into_owned())
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    std::thread::Builder::new().name(name.into()).spawn(f).expect("建不了录屏线程")
}

/// 停下线程、写好文件尾（阻塞）。返回编码过程中的错误。
fn finish(rec: Recording) -> (PathBuf, Option<String>) {
    // 先让时钟停在按下停止的这一刻：编码线程收尾慢（GIF）也不会把最后一帧拖长
    rec.shared.clock.lock().expect("录屏时钟锁").set_paused(true);
    rec.shared.stop.store(true, Ordering::SeqCst);
    for t in rec.threads {
        let _ = t.join();
    }
    if let Some(m) = rec.mp4 {
        if let Some(w) = m.lock().expect("MP4 锁").take() {
            if let Err(e) = w.finish() {
                rec.shared.fail(e);
            }
        }
    }
    let err = rec.shared.error.lock().expect("录屏错误锁").take();
    (rec.path, err)
}

fn take<R: Runtime>(app: &AppHandle<R>) -> Option<Recording> {
    app.state::<RecordState>().inner.lock().expect("录屏锁").take()
}

/// 截图意外结束（Alt+F4 等）时：停下录制并保留已经录到的内容。
pub fn stop_and_keep<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rec) = take(app) {
        std::thread::spawn(move || {
            let (path, err) = finish(rec);
            if let Some(e) = err {
                crate::log_warn!("录屏收尾出错（{}）：{e}", path.display());
            }
        });
    }
}

/// 暂停 / 继续。
#[tauri::command]
pub fn record_pause(state: State<'_, RecordState>, paused: bool) {
    if let Some(r) = state.inner.lock().expect("录屏锁").as_ref() {
        r.shared.clock.lock().expect("录屏时钟锁").set_paused(paused);
    }
}

/// 停止。`save` 为 false 时放弃（删掉文件）。保存成功返回文件路径，并把文件放进剪贴板。
/// 遮罩回到可以点的状态，由前端显示一下「已保存」后再结束截图。
#[tauri::command]
pub async fn record_stop(app: AppHandle, save: bool) -> Result<Option<String>, String> {
    let rec = take(&app).ok_or_else(|| tr("没有在录屏"))?;
    live::exit(&app, !save);
    let (path, err) = tauri::async_runtime::spawn_blocking(move || finish(rec)).await.map_err(|e| e.to_string())?;
    if !save {
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    }
    if let Some(e) = err {
        let _ = std::fs::remove_file(&path);
        return Err(trf("录屏保存失败：{e}", &[("e", &e.to_string())]));
    }
    if let Err(e) = clipboard::set_file(&path) {
        crate::log_warn!("录屏文件放进剪贴板失败：{e}");
    }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_is_made_even() {
        let r = even(Region { x: 3, y: 5, width: 801, height: 451 });
        assert_eq!((r.x, r.y, r.width, r.height), (3, 5, 800, 450));
    }

    #[test]
    fn fps_is_capped_per_format() {
        assert_eq!(effective_fps(RecordFormat::Gif, 60), 30);
        assert_eq!(effective_fps(RecordFormat::Mp4, 60), 60);
        assert_eq!(effective_fps(RecordFormat::Mp4, 0), 1);
    }

    #[test]
    fn clock_excludes_pauses() {
        let mut c = Clock { start: Instant::now() - Duration::from_secs(10), paused: Duration::from_secs(3), since: None };
        let t = c.now();
        assert!(t >= Duration::from_secs(7) && t < Duration::from_millis(7100));
        c.set_paused(true);
        let a = c.now();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(c.now(), a, "暂停时不走");
        c.set_paused(false);
        assert!(c.now() < a + Duration::from_millis(20));
    }
}
