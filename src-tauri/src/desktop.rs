use tauri::WebviewWindow;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{IShellDispatch, Shell};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetAncestor, GetShellWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, SetParent, SetWindowLongW, SetWindowPos, ShowWindow,
    GA_PARENT, GWL_STYLE, HWND_TOP, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WS_CHILD, WS_POPUP,
};

fn hwnd(window: &WebviewWindow) -> Result<HWND, String> {
    window
        .hwnd()
        .map(|handle| HWND(handle.0))
        .map_err(|error| error.to_string())
}

fn icon_host() -> Option<HWND> {
    // Explorer's icon view can live in Progman or in a top-level WorkerW.
    // The wallpaper-only WorkerW sits behind the icon view and cannot host input.
    unsafe {
        let shell = GetShellWindow();
        if shell.0.is_null() {
            return None;
        }
        let mut shell_pid = 0;
        GetWindowThreadProcessId(shell, Some(&mut shell_pid));
        if shell_pid == 0 {
            return None;
        }
        let mut search = HostSearch {
            shell_pid,
            host: HWND::default(),
        };
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(find_icon_host),
            LPARAM((&mut search as *mut HostSearch) as isize),
        );
        (!search.host.0.is_null()).then_some(search.host)
    }
}

struct HostSearch {
    shell_pid: u32,
    host: HWND,
}

unsafe extern "system" fn find_icon_host(
    window: HWND,
    state: LPARAM,
) -> windows::Win32::Foundation::BOOL {
    let search = &mut *(state.0 as *mut HostSearch);
    let mut pid = 0;
    GetWindowThreadProcessId(window, Some(&mut pid));
    if pid == search.shell_pid
        && IsWindowVisible(window).as_bool()
        && FindWindowExW(window, None, w!("SHELLDLL_DefView"), None).is_ok()
    {
        search.host = window;
        return false.into();
    }
    true.into()
}

fn client_position(parent: HWND, x: i32, y: i32) -> Result<POINT, String> {
    let mut point = POINT { x, y };
    if !unsafe { ScreenToClient(parent, &mut point) }.as_bool() {
        return Err("无法换算便签的桌面坐标".into());
    }
    Ok(point)
}

fn attach(window: &WebviewWindow) -> Result<(), String> {
    let handle = hwnd(window)?;
    let host = icon_host().ok_or("找不到 Windows 桌面")?;
    unsafe {
        if GetAncestor(handle, GA_PARENT) == host {
            return Ok(());
        }
        let mut rect = RECT::default();
        GetWindowRect(handle, &mut rect).map_err(|error| error.to_string())?;
        let point = client_position(host, rect.left, rect.top)?;
        let old_style = GetWindowLongW(handle, GWL_STYLE);
        SetWindowLongW(
            handle,
            GWL_STYLE,
            (old_style & !(WS_POPUP.0 as i32)) | WS_CHILD.0 as i32,
        );
        // Check the actual parent. A zero return can also mean no previous parent.
        let result = SetParent(handle, host);
        if GetAncestor(handle, GA_PARENT) != host {
            SetWindowLongW(handle, GWL_STYLE, old_style);
            return Err(result
                .err()
                .map_or_else(|| "无法挂载桌面便签".into(), |error| error.to_string()));
        }
        SetWindowPos(
            handle,
            HWND_TOP,
            point.x,
            point.y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_FRAMECHANGED,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn show(window: &WebviewWindow) -> Result<(), String> {
    attach(window)?;
    let handle = hwnd(window)?;
    unsafe {
        // Raise only among the desktop's children, leaving regular apps above it.
        SetWindowPos(
            handle,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|error| error.to_string())?;
        let _ = ShowWindow(handle, SW_SHOWNA);
    }
    Ok(())
}

pub fn hide(window: &WebviewWindow) -> Result<(), String> {
    // Tauri's hidden top-level state does not track a child shown through Win32.
    unsafe {
        let _ = ShowWindow(hwnd(window)?, SW_HIDE);
    }
    Ok(())
}

pub fn reveal_desktop() -> windows::core::Result<()> {
    unsafe {
        let shell: IShellDispatch = CoCreateInstance(&Shell, None, CLSCTX_INPROC_SERVER)?;
        shell.MinimizeAll()
    }
}

pub fn set_bounds(
    window: &WebviewWindow,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let handle = hwnd(window)?;
    unsafe {
        let parent = GetAncestor(handle, GA_PARENT);
        let point = client_position(parent, x, y)?;
        SetWindowPos(
            handle,
            None,
            point.x,
            point.y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(|error| error.to_string())
    }
}

pub fn move_to(window: &WebviewWindow, x: i32, y: i32) -> Result<(), String> {
    let handle = hwnd(window)?;
    unsafe {
        let point = client_position(GetAncestor(handle, GA_PARENT), x, y)?;
        SetWindowPos(
            handle,
            None,
            point.x,
            point.y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(|error| error.to_string())
    }
}
