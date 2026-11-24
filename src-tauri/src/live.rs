//! 「透出真实画面」模式：长截图（M9.1）和录屏（M9.2）共用。
//!
//! - 遮罩不再显示冻结帧（前端负责），并且不让截屏抓到自己（WDA_EXCLUDEFROMCAPTURE），
//!   所以每一帧抓到的都是遮罩下面的真实内容；
//! - 遮罩对鼠标透明，滚轮和点击落到下面的程序上；只有指针移到前端报来的区域（工具条）上时才接收鼠标
//!   ——由使用它的后台线程每一帧调用 [`Live::poll`] 切换；键盘焦点留在遮罩上；
//! - 其他显示器上的遮罩先停到屏幕外，让出那几块屏（取消时放回来）。

use std::sync::{Arc, Mutex};

use snapmind_platform::cursor;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};

use crate::export::Region;
use crate::overlay;

#[derive(Default)]
pub struct LiveState {
    current: Mutex<Option<Arc<Live>>>,
}

pub struct Live {
    window: WebviewWindow,
    /// 本屏左上角（虚拟桌面物理像素）
    mon: (i32, i32),
    /// (已停止, 当前是否穿透)。切换穿透时拿着这把锁，停止后线程不会再把遮罩设回穿透
    flags: Mutex<(bool, bool)>,
    /// 要接收鼠标的区域（本屏物理像素）
    hit: Mutex<Vec<Region>>,
}

fn contains(r: &Region, x: i32, y: i32) -> bool {
    x >= r.x as i32 && y >= r.y as i32 && x < (r.x + r.width) as i32 && y < (r.y + r.height) as i32
}

impl Live {
    /// 进入透出模式（已经在透出模式就先退出原来的）。
    pub fn enter(app: &AppHandle, window: &WebviewWindow, mon: (i32, i32)) -> Arc<Live> {
        exit(app, false);
        if let Some(h) = crate::wm::native_handle(window) {
            let _ = app.run_on_main_thread(move || {
                if !snapmind_platform::window::exclude_from_capture(h, true) {
                    crate::log_warn!("系统不支持把遮罩排除在截屏之外（需要 Windows 10 2004 及以上）");
                }
            });
        }
        let _ = window.set_ignore_cursor_events(true);
        overlay::park_except(app, window.label());
        let live = Arc::new(Live { window: window.clone(), mon, flags: Mutex::new((false, true)), hit: Mutex::new(Vec::new()) });
        *app.state::<LiveState>().current.lock().expect("透出模式锁") = Some(live.clone());
        live
    }

    /// 后台线程每一帧调用：按指针位置切换鼠标穿透。已停止时返回 false，线程应当退出。
    pub fn poll(&self) -> bool {
        let mut flags = self.flags.lock().expect("透出模式标志锁");
        if flags.0 {
            return false;
        }
        if let Some((cx, cy)) = cursor::position() {
            let over = self.hit.lock().expect("透出模式区域锁").iter().any(|r| contains(r, cx - self.mon.0, cy - self.mon.1));
            if over == flags.1 {
                flags.1 = !over;
                let _ = self.window.set_ignore_cursor_events(flags.1);
            }
        }
        true
    }

    /// 停止：恢复接收鼠标、允许被截屏。可以在任何线程调用（窗口操作交给主线程，不等待）。
    fn stop(&self) {
        let mut flags = self.flags.lock().expect("透出模式标志锁");
        flags.0 = true;
        let _ = self.window.set_ignore_cursor_events(false);
        if let Some(h) = crate::wm::native_handle(&self.window) {
            let _ = self.window.app_handle().run_on_main_thread(move || {
                snapmind_platform::window::exclude_from_capture(h, false);
            });
        }
    }
}

/// 退出透出模式。`unpark`：把其他显示器的遮罩放回去（回到普通截图编辑态时）；结束截图时不需要。
pub fn exit<R: Runtime>(app: &AppHandle<R>, unpark: bool) {
    let live = app.state::<LiveState>().current.lock().expect("透出模式锁").take();
    if let Some(l) = live {
        l.stop();
        if unpark {
            overlay::unpark(app);
        }
    }
}

/// 前端报来要接收鼠标的区域（工具条），本屏物理像素。
#[tauri::command]
pub fn live_hit(state: State<'_, LiveState>, rects: Vec<Region>) {
    if let Some(l) = state.current.lock().expect("透出模式锁").as_ref() {
        *l.hit.lock().expect("透出模式区域锁") = rects;
    }
}
