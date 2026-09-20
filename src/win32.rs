// Hand-written Win32 FFI: window + GL context, raw keyboard/mouse, timing.
#![allow(non_snake_case, dead_code)]

use std::cell::Cell;
use std::os::raw::c_void;
use std::path::PathBuf;

pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HGLRC = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HICON = *mut c_void;
pub type HMENU = *mut c_void;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type BOOL = i32;
pub type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

pub const WM_DESTROY: u32 = 2;
pub const WM_SIZE: u32 = 5;
pub const WM_ACTIVATE: u32 = 6;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUIT: u32 = 0x0012;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_MOUSEWHEEL: u32 = 0x020A;

pub const PM_REMOVE: u32 = 1;

pub const CS_HREDRAW: u32 = 0x0002;
pub const CS_VREDRAW: u32 = 0x0001;
pub const CS_OWNDC: u32 = 0x0020;

pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;

pub const PFD_DRAW_TO_WINDOW: u32 = 0x0000_0004;
pub const PFD_SUPPORT_OPENGL: u32 = 0x0000_0001;
pub const PFD_DOUBLEBUFFER: u32 = 0x0000_0002;

pub const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
pub const SW_SHOW: i32 = 5;
pub const IDC_ARROW: usize = 32512;
pub const FALSE: BOOL = 0;
pub const TRUE: BOOL = 1;

#[repr(C)]
pub struct WNDCLASSW {
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
pub struct PIXELFORMATDESCRIPTOR {
    pub nSize: u16,
    pub nVersion: u16,
    pub dwFlags: u32,
    pub iPixelType: u8,
    pub cColorBits: u8,
    pub cRedBits: u8,
    pub cRedShift: u8,
    pub cGreenBits: u8,
    pub cGreenShift: u8,
    pub cBlueBits: u8,
    pub cBlueShift: u8,
    pub cAlphaBits: u8,
    pub cAlphaShift: u8,
    pub cAccumBits: u8,
    pub cAccumRedBits: u8,
    pub cAccumGreenBits: u8,
    pub cAccumBlueBits: u8,
    pub cAccumAlphaBits: u8,
    pub cDepthBits: u8,
    pub cStencilBits: u8,
    pub cAuxBuffers: u8,
    pub iLayerType: u8,
    pub bReserved: u8,
    pub dwLayerMask: u32,
    pub dwVisibleMask: u32,
    pub dwDamageMask: u32,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    fn GetModuleFileNameW(inst: HINSTANCE, buf: *mut u16, len: u32) -> u32;
    fn QueryPerformanceCounter(out: *mut i64) -> BOOL;
    fn QueryPerformanceFrequency(out: *mut i64) -> BOOL;
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HMENU,
        inst: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    fn GetDC(hwnd: HWND) -> HDC;
    fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
    fn PeekMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32, remove: u32) -> BOOL;
    fn TranslateMessage(msg: *const MSG) -> BOOL;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: HWND) -> BOOL;
    fn LoadCursorW(inst: HINSTANCE, id: usize) -> HCURSOR;
    fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    fn ClientToScreen(hwnd: HWND, pt: *mut POINT) -> BOOL;
    fn GetCursorPos(pt: *mut POINT) -> BOOL;
    fn SetCursorPos(x: i32, y: i32) -> BOOL;
    fn ShowCursor(show: BOOL) -> i32;
    fn ClipCursor(rect: *const RECT) -> BOOL;
    fn GetAsyncKeyState(vk: i32) -> i16;
    fn SetWindowTextW(hwnd: HWND, text: *const u16) -> BOOL;
    fn MessageBoxW(hwnd: HWND, text: *const u16, caption: *const u16, style: u32) -> i32;
    fn AdjustWindowRect(rect: *mut RECT, style: u32, menu: BOOL) -> BOOL;
    fn ScreenToClient(hwnd: HWND, pt: *mut POINT) -> BOOL;
}

#[cfg(windows)]
#[link(name = "gdi32")]
extern "system" {
    fn ChoosePixelFormat(hdc: HDC, pfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn SetPixelFormat(hdc: HDC, format: i32, pfd: *const PIXELFORMATDESCRIPTOR) -> BOOL;
    fn SwapBuffers(hdc: HDC) -> BOOL;
}

#[cfg(windows)]
#[link(name = "opengl32")]
extern "system" {
    fn wglCreateContext(hdc: HDC) -> HGLRC;
    fn wglMakeCurrent(hdc: HDC, ctx: HGLRC) -> BOOL;
    fn wglGetProcAddress(name: *const u8) -> *mut c_void;
}

// Linker stubs so `cargo test --features fficheck` can build on a non-Windows
// host. These are never called there: tests only exercise pure game logic.
#[cfg(not(windows))]
mod stubs {
    use super::*;
    macro_rules! stub {
        ($($name:ident($($arg:ty),*) -> $ret:ty);* $(;)?) => {
            $(pub unsafe extern "system" fn $name($(_: $arg),*) -> $ret { unreachable!() })*
        };
    }
    stub! {
        GetModuleHandleW(*const u16) -> HINSTANCE;
        GetModuleFileNameW(HINSTANCE, *mut u16, u32) -> u32;
        QueryPerformanceCounter(*mut i64) -> BOOL;
        QueryPerformanceFrequency(*mut i64) -> BOOL;
        RegisterClassW(*const WNDCLASSW) -> u16;
        CreateWindowExW(u32, *const u16, *const u16, u32, i32, i32, i32, i32, HWND, HMENU, HINSTANCE, *mut c_void) -> HWND;
        DefWindowProcW(HWND, u32, WPARAM, LPARAM) -> LRESULT;
        ShowWindow(HWND, i32) -> BOOL;
        GetDC(HWND) -> HDC;
        ReleaseDC(HWND, HDC) -> i32;
        PeekMessageW(*mut MSG, HWND, u32, u32, u32) -> BOOL;
        TranslateMessage(*const MSG) -> BOOL;
        DispatchMessageW(*const MSG) -> LRESULT;
        PostQuitMessage(i32) -> ();
        DestroyWindow(HWND) -> BOOL;
        LoadCursorW(HINSTANCE, usize) -> HCURSOR;
        GetClientRect(HWND, *mut RECT) -> BOOL;
        ClientToScreen(HWND, *mut POINT) -> BOOL;
        GetCursorPos(*mut POINT) -> BOOL;
        SetCursorPos(i32, i32) -> BOOL;
        ShowCursor(BOOL) -> i32;
        ClipCursor(*const RECT) -> BOOL;
        GetAsyncKeyState(i32) -> i16;
        SetWindowTextW(HWND, *const u16) -> BOOL;
        MessageBoxW(HWND, *const u16, *const u16, u32) -> i32;
        AdjustWindowRect(*mut RECT, u32, BOOL) -> BOOL;
        ScreenToClient(HWND, *mut POINT) -> BOOL;
        ChoosePixelFormat(HDC, *const PIXELFORMATDESCRIPTOR) -> i32;
        SetPixelFormat(HDC, i32, *const PIXELFORMATDESCRIPTOR) -> BOOL;
        SwapBuffers(HDC) -> BOOL;
        wglCreateContext(HDC) -> HGLRC;
        wglMakeCurrent(HDC, HGLRC) -> BOOL;
        wglGetProcAddress(*const u8) -> *mut c_void
    }
}

#[cfg(not(windows))]
use stubs::*;

pub fn wgl_get_proc_address(name: *const u8) -> *mut c_void {
    unsafe { wglGetProcAddress(name) }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

thread_local! {
    static SIZE: Cell<(i32, i32)> = Cell::new((1280, 720));
    static CAPTURED: Cell<bool> = Cell::new(false);
    static QPC_FREQ: Cell<f64> = Cell::new(1.0);
    static CHARS: std::cell::RefCell<Vec<char>> = std::cell::RefCell::new(Vec::new());
    static WHEEL: Cell<f32> = Cell::new(0.0);
}

pub struct Window {
    pub hwnd: HWND,
    pub hdc: HDC,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            let w = (lp & 0xFFFF) as i32;
            let h = ((lp >> 16) & 0xFFFF) as i32;
            if w > 0 && h > 0 {
                SIZE.with(|s| s.set((w, h)));
            }
            0
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wp as u32) {
                CHARS.with(|q| q.borrow_mut().push(c));
            }
            0
        }
        WM_MOUSEWHEEL => {
            let d = ((wp >> 16) & 0xFFFF) as u16 as i16 as f32 / 120.0;
            WHEEL.with(|w| w.set(w.get() + d));
            0
        }
        WM_ACTIVATE => {
            if (wp & 0xFFFF) == 0 {
                release_capture();
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn release_capture() {
    if CAPTURED.with(|c| c.get()) {
        unsafe {
            ShowCursor(TRUE);
            ClipCursor(std::ptr::null());
        }
        CAPTURED.with(|c| c.set(false));
    }
}

pub fn set_captured(on: bool) {
    if on == CAPTURED.with(|c| c.get()) {
        return;
    }
    CAPTURED.with(|c| c.set(on));
    unsafe {
        if on {
            ShowCursor(FALSE);
            clip_to_client();
            center_cursor();
        } else {
            ShowCursor(TRUE);
            ClipCursor(std::ptr::null());
        }
    }
}

pub fn cursor_captured() -> bool {
    CAPTURED.with(|c| c.get())
}

unsafe fn client_rect_screen() -> RECT {
    let hwnd = CURRENT_HWND.with(|c| c.get());
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut r);
    let mut tl = POINT { x: r.left, y: r.top };
    ClientToScreen(hwnd, &mut tl);
    let mut br = POINT { x: r.right, y: r.bottom };
    ClientToScreen(hwnd, &mut br);
    RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y }
}

unsafe fn center_cursor() {
    let r = client_rect_screen();
    let cx = (r.left + r.right) / 2;
    let cy = (r.top + r.bottom) / 2;
    SetCursorPos(cx, cy);
}

unsafe fn clip_to_client() {
    let r = client_rect_screen();
    ClipCursor(&r);
}

thread_local! {
    static CURRENT_HWND: Cell<HWND> = Cell::new(std::ptr::null_mut());
}

/// Mouse movement since last frame, in pixels (only when captured).
pub fn mouse_delta() -> (f32, f32) {
    if !CAPTURED.with(|c| c.get()) {
        return (0.0, 0.0);
    }
    unsafe {
        let r = client_rect_screen();
        let cx = (r.left + r.right) / 2;
        let cy = (r.top + r.bottom) / 2;
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let dx = pt.x - cx;
        let dy = pt.y - cy;
        SetCursorPos(cx, cy);
        (dx as f32, dy as f32)
    }
}

pub fn pump() -> bool {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) == 0 {
                return true;
            }
            if msg.message == WM_QUIT {
                return false;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn client_size() -> (i32, i32) {
    SIZE.with(|s| s.get())
}

pub fn swap(hdc: HDC) {
    unsafe {
        SwapBuffers(hdc);
    }
}

pub fn set_title(hwnd: HWND, text: &str) {
    unsafe {
        SetWindowTextW(hwnd, wide(text).as_ptr());
    }
}

pub fn key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// Characters typed since the last call (chat / text fields).
pub fn take_chars() -> Vec<char> {
    CHARS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Accumulated wheel notches since the last call (+ = up).
pub fn take_wheel() -> f32 {
    WHEEL.with(|w| w.replace(0.0))
}

/// Mouse position in client coordinates (pixels).
pub fn cursor_pos() -> (f32, f32) {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let hwnd = CURRENT_HWND.with(|c| c.get());
        ScreenToClient(hwnd, &mut pt);
        (pt.x as f32, pt.y as f32)
    }
}

pub fn qpc_counter() -> i64 {
    let mut v = 0i64;
    unsafe {
        QueryPerformanceCounter(&mut v);
    }
    v
}

pub fn qpc_dt(prev: i64, now: i64) -> f32 {
    (QPC_FREQ.with(|f| f.get()) * (now - prev).max(0) as f64) as f32
}

pub fn exe_dir() -> PathBuf {
    unsafe {
        let mut buf = [0u16; 1024];
        let n = GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), 1024) as usize;
        let n = n.min(1024);
        let s = String::from_utf16_lossy(&buf[..n]);
        let mut p = PathBuf::from(s);
        p.pop();
        p
    }
}

pub fn fatal(msg: &str) -> ! {
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(msg).as_ptr(),
            wide("RustVoxel - error").as_ptr(),
            0x10, // MB_ICONERROR
        );
    }
    std::process::exit(1);
}

pub fn init(title: &str, w: i32, h: i32) -> Result<Window, String> {
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class_name = wide("RustVoxelWnd");
        let wc = WNDCLASSW {
            style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return Err("RegisterClassW failed".into());
        }

        let mut freq = 0i64;
        QueryPerformanceFrequency(&mut freq);
        QPC_FREQ.with(|f| f.set(1.0 / freq.max(1) as f64));

        // client area of exactly w x h
        let mut rect = RECT { left: 0, top: 0, right: w, bottom: h };
        AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, FALSE);

        let t = wide(title);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            t.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            inst,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowExW failed".into());
        }
        CURRENT_HWND.with(|c| c.set(hwnd));
        ShowWindow(hwnd, SW_SHOW);

        let hdc = GetDC(hwnd);
        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
        pfd.nVersion = 1;
        pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
        pfd.iPixelType = 0; // RGBA
        pfd.cColorBits = 24;
        pfd.cDepthBits = 24;
        pfd.iLayerType = 0; // main plane
        let pf = ChoosePixelFormat(hdc, &pfd);
        if pf == 0 || SetPixelFormat(hdc, pf, &pfd) == 0 {
            return Err("pixel format failed".into());
        }
        let ctx = wglCreateContext(hdc);
        if ctx.is_null() || wglMakeCurrent(hdc, ctx) == 0 {
            return Err("OpenGL context failed".into());
        }

        Ok(Window { hwnd, hdc })
    }
}
