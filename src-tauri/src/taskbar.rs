// Windows 任务栏钉住条定位：获取任务栏 RECT，计算左/中/右定位坐标
// 非目标平台提供空实现

#[cfg(windows)]
pub fn taskbar_anchor(position: &str, bar_width: i32, bar_height: i32) -> (i32, i32) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};
    use windows::core::w;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO,
        MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::{ABM_GETTASKBARPOS, SHAppBarMessage};

    let h = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) };
    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };

    if let Ok(h) = h {
        // 优先用 APPBARDATA 取任务栏矩形
        use windows::Win32::UI::Shell::APPBARDATA;
        let mut abd: APPBARDATA = unsafe { std::mem::zeroed() };
        abd.cbSize = std::mem::size_of::<APPBARDATA>() as u32;
        if unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut abd as *mut _ as _) } != 0 {
            rect = abd.rc;
        } else if unsafe { GetWindowRect(h, &mut rect).is_err() } {
            rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        }
    }

    // 取包含任务栏的显示器工作区
    let hmon: HMONITOR = unsafe { MonitorFromWindow(h.unwrap_or_default(), MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    let _ = unsafe { GetMonitorInfoW(hmon, &mut mi) };
    let work = mi.rcWork;

    // 垂直贴任务栏上边缘
    let y = work.bottom - bar_height;

    // 水平三档
    let work_w = work.right - work.left;
    let x = match position {
        "left" => work.left + 8,
        "center" => work.left + (work_w - bar_width) / 2,
        _ => work.left + work_w - bar_width - 8,
    };
    (x, y)
}

#[cfg(not(windows))]
pub fn taskbar_anchor(_position: &str, _bar_width: i32, _bar_height: i32) -> (i32, i32) {
    // 非 Windows 平台退化为右下角
    (100, 100)
}
