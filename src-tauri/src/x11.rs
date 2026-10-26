//! Linux(X11)平台:按 EWMH 规范向窗口管理器请求「前置并聚焦」主窗口。
//!
//! 为什么不走 tao 的 set_focus / set_always_on_top(ukui-kwin 实测,勿轻易改回):
//!
//! 1. tao(Linux/GTK 后端)的 set_focus 最终只是 `gtk_window_present_with_time(GDK_CURRENT_TIME)`,
//!    且窗口「当前不可见」时请求被静默丢弃;UKUI 这类桌面的「防抢焦点」机制
//!    还会忽略已映射窗口的这种激活请求——表现为窗口被其它应用盖住时,
//!    从托盘唤出既提不到最前、也拿不到焦点。
//! 2. tao 的 set_always_on_top 走 GTK 的 set_keep_above,在 UKUI 上同样不一定被理会,
//!    「临时置顶再取消」的变通也因此不可靠(kylinpaste 实测:置顶不生效的表现就是
//!    "点了别处窗口就被压到后面")。
//!
//! 这里直接向根窗口发 EWMH `_NET_ACTIVE_WINDOW`(source=2,表示用户主动请求,
//! 窗口管理器一般会绕过防抢焦点限制放行),并轮询 `_NET_ACTIVE_WINDOW` 确认;
//! WM 不响应时退回 XSetInputFocus 兜底。做法与 kylinpaste(同为 Tauri 应用,
//! 已在 UKUI 上验证)一致。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, InputFocus, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

use crate::logfile;

type XResult<T> = Result<T, String>;

fn connect() -> XResult<(RustConnection, Window)> {
    let (conn, screen_num) =
        RustConnection::connect(None).map_err(|e| format!("连接 X11 失败: {e}"))?;
    let root = conn.setup().roots[screen_num].root;
    Ok((conn, root))
}

/// 复用的 X11 连接(连同它对应的 root 窗口)。
///
/// `RustConnection` 本身按多线程使用设计(内部自带锁),共享一条即可。
/// 连接出错(例如 X 会话重启)时丢掉缓存,下一次调用自动重连。
static SHARED_CONN: Lazy<Mutex<Option<Arc<SharedConn>>>> = Lazy::new(|| Mutex::new(None));

struct SharedConn {
    conn: RustConnection,
    root: Window,
}

/// 取一条可复用的连接。拿到的是 `Arc`,**调用期间不要持有 `SHARED_CONN` 的锁**
/// (X11 往返可能上百毫秒,持锁会挡住别的线程)。
fn shared_conn() -> XResult<Arc<SharedConn>> {
    let mut guard = SHARED_CONN.lock().map_err(|_| "X11 连接状态锁失败")?;
    if guard.is_none() {
        let (conn, root) = connect()?;
        *guard = Some(Arc::new(SharedConn { conn, root }));
    }
    Ok(Arc::clone(guard.as_ref().expect("刚建过")))
}

/// 连接可能已经坏了:丢掉缓存,让下一次调用重新连
fn forget_shared_conn() {
    if let Ok(mut guard) = SHARED_CONN.lock() {
        *guard = None;
    }
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> XResult<Atom> {
    conn.intern_atom(false, name)
        .map_err(|e| format!("X11 请求失败: {e}"))?
        .reply()
        .map_err(|e| format!("X11 响应失败: {e}"))
        .map(|reply| reply.atom)
}

/// 读取根窗口上的 _NET_ACTIVE_WINDOW(EWMH 约定的「当前活动窗口」)
fn active_window(conn: &RustConnection, root: Window) -> Option<Window> {
    let atom = intern_atom(conn, b"_NET_ACTIVE_WINDOW").ok()?;
    conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
        .filter(|win| *win != 0)
}

/// 读取窗口的 _NET_WM_PID,用于判断某个窗口是不是本应用的
fn window_pid(conn: &RustConnection, win: Window) -> Option<u32> {
    let atom = intern_atom(conn, b"_NET_WM_PID").ok()?;
    conn.get_property(false, win, atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
}

/// 读取窗口的 WM_CLASS(class 部分)。WM_CLASS 是一串 `实例\0类\0`,取更规范的类名;
/// 窗口没有 WM_CLASS 时返回 None。
fn window_class(conn: &RustConnection, win: Window) -> Option<String> {
    let reply = conn
        .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()?
        .reply()
        .ok()?;
    let parts: Vec<String> = reply
        .value
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    parts.last().or_else(|| parts.first()).cloned()
}

/// 读取根窗口的 _NET_CLIENT_LIST(EWMH 维护的「受管理窗口列表」)
fn client_list(conn: &RustConnection, root: Window) -> Vec<Window> {
    let Ok(atom) = intern_atom(conn, b"_NET_CLIENT_LIST") else {
        return Vec::new();
    };
    let Some(reply) = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 256)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
    else {
        return Vec::new();
    };
    reply.value32().map(|it| it.collect()).unwrap_or_default()
}

/// 找本应用主窗口的 X11 窗口 id。
///
/// 呼出/聚焦都要走 EWMH(见模块注释),所以必须先拿到窗口 id。
/// 按 `_NET_WM_PID + WM_CLASS` 匹配;class 读不到时退回只比 pid。
pub fn main_window_xid() -> Option<Window> {
    let shared = shared_conn().ok()?;
    find_main_window(&shared.conn, shared.root, std::process::id())
}

/// 在「受管理窗口列表」里找本应用的主窗口(按 `_NET_WM_PID + WM_CLASS` 匹配)
fn find_main_window(conn: &RustConnection, root: Window, me: u32) -> Option<Window> {
    client_list(conn, root).into_iter().find(|&win| {
        window_pid(conn, win) == Some(me)
            && window_class(conn, win)
                .map(|class| class.to_ascii_lowercase().contains("loudmail"))
                .unwrap_or(true)
    })
}

fn wait_active(conn: &RustConnection, root: Window, win: Window, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if active_window(conn, root) == Some(win) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 请求窗口管理器前置并聚焦 `win`,返回是否观察到主窗口确实成了活动窗口。
///
/// 先按 EWMH 发 _NET_ACTIVE_WINDOW(source=2,表示用户主动请求,可绕过防抢焦点限制),
/// 窗口管理器不理会时再退回 XSetInputFocus。
pub fn activate_window(win: Window) -> bool {
    let shared = match shared_conn() {
        Ok(shared) => shared,
        Err(e) => {
            logfile::warn(format!("[X11] {e}"));
            return false;
        }
    };
    let (conn, root) = (&shared.conn, shared.root);

    let atom = match intern_atom(conn, b"_NET_ACTIVE_WINDOW") {
        Ok(atom) => atom,
        Err(e) => {
            logfile::warn(format!("[X11] {e}"));
            forget_shared_conn();
            return false;
        }
    };

    // EWMH 约定:data.l[0]=时间戳,data.l[1]=来源(2 = 分页器/用户主动请求,
    // 窗口管理器一般会绕过防抢焦点限制放行),其余字段未用
    let event = ClientMessageEvent::new(32, win, atom, [CURRENT_TIME, 2u32, 0, 0, 0]);
    let sent = conn
        .send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .map_err(|e| e.to_string())
        .and_then(|cookie| cookie.check().map_err(|e| e.to_string()));
    if let Err(e) = sent {
        logfile::warn(format!("[X11] 发送 _NET_ACTIVE_WINDOW 失败: {e}"));
    }
    let _ = conn.flush();

    if wait_active(conn, root, win, Duration::from_millis(400)) {
        return true;
    }

    logfile::info("[X11] 窗口管理器未响应 _NET_ACTIVE_WINDOW,回退 XSetInputFocus");
    if let Err(e) = conn
        .set_input_focus(InputFocus::POINTER_ROOT, win, CURRENT_TIME)
        .map_err(|e| e.to_string())
        .and_then(|cookie| cookie.check().map_err(|e| e.to_string()))
    {
        logfile::warn(format!("[X11] XSetInputFocus 失败: {e}"));
        forget_shared_conn();
        return false;
    }
    let _ = conn.flush();
    wait_active(conn, root, win, Duration::from_millis(200))
}
