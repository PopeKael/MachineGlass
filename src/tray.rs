//! Native tray lifetime and events stay on eframe's Windows message-loop thread.
use crate::config::log;
use eframe::egui::{Context, IconData, ViewportCommand};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsWindow, IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

pub fn window_icon() -> IconData {
    IconData {
        rgba: include_bytes!("../assets/stethoscope.rgba").to_vec(),
        width: 64,
        height: 64,
    }
}

pub struct Tray {
    icon: Option<TrayIcon>,
    quitting: Arc<AtomicBool>,
}
impl Tray {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, Box<dyn std::error::Error>> {
        let ctx = &cc.egui_ctx;
        let RawWindowHandle::Win32(window) = cc.window_handle()?.as_raw() else {
            return Err("Native Windows window handle unavailable".into());
        };
        // Store the borrowed HWND as an integer to share with static event callbacks.
        // It remains owned by eframe; handlers are cleared before window destruction.
        let hwnd = window.hwnd.get();
        let menu = Menu::new();
        let show = MenuItem::new("Show MachineGlass", true, None);
        let hide = MenuItem::new("Hide to tray", true, None);
        let quit = MenuItem::new("Quit MachineGlass", true, None);
        menu.append_items(&[&show, &hide, &PredefinedMenuItem::separator(), &quit])?;
        let show_id = show.id().clone();
        let hide_id = hide.id().clone();
        let quit_id = quit.id().clone();
        let data = window_icon();
        let icon = TrayIconBuilder::new()
            .with_tooltip("MachineGlass — system observability")
            .with_icon(Icon::from_rgba(data.rgba, data.width, data.height)?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;
        let tray_id = icon.id().clone();
        let quitting = Arc::new(AtomicBool::new(false));
        let flag = quitting.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                restore(&menu_ctx, hwnd);
            } else if event.id == hide_id {
                menu_ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                log("Window hidden from tray menu; collection continues");
            } else if event.id == quit_id {
                log("Quit requested from tray menu; queuing visibility and close commands");
                flag.store(true, Ordering::Relaxed);
                // Hidden windows do not reliably receive redraws. Show the HWND
                // immediately before queuing commands handled by eframe's next frame.
                restore(&menu_ctx, hwnd);
                menu_ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            menu_ctx.request_repaint();
        }));
        let click_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if event.id() == &tray_id
                && matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                )
            {
                restore(&click_ctx, hwnd);
            }
        }));
        log("Native stethoscope tray icon registered");
        Ok(Self {
            icon: Some(icon),
            quitting,
        })
    }
    pub fn quitting(&self) -> bool {
        self.quitting.load(Ordering::Relaxed)
    }
}
fn restore(ctx: &Context, hwnd: isize) {
    // SAFETY: called on the native message-loop thread for eframe's borrowed
    // window. Check lifetime before use; neither function takes ownership.
    unsafe {
        let window = hwnd as windows_sys::Win32::Foundation::HWND;
        if IsWindow(window) == 0 {
            log("Tray restore failed: native window no longer exists");
            return;
        }
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
        log(if IsWindowVisible(window) != 0 {
            "Tray restore: native window is visible"
        } else {
            "Tray restore failed: native window remains hidden"
        });
    }
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
    ctx.request_repaint();
}
impl Drop for Tray {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
        // TrayIcon's Drop calls Shell_NotifyIcon(NIM_DELETE).
        log("Removing native tray icon");
        let began = std::time::Instant::now();
        drop(self.icon.take());
        log(&format!(
            "Native tray icon removed in {:.1} ms",
            began.elapsed().as_secs_f64() * 1000.0
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SW_HIDE, WS_OVERLAPPEDWINDOW,
    };

    #[test]
    fn restore_shows_hidden_native_window_without_an_egui_frame() {
        // SAFETY: STATIC is a built-in Windows class; the window is created,
        // inspected and destroyed on this test's thread, with no borrowed data.
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                100,
                100,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "Native test window creation failed");
            ShowWindow(hwnd, SW_HIDE);
            assert_eq!(IsWindowVisible(hwnd), 0);
            restore(&Context::default(), hwnd as isize);
            let visible = IsWindowVisible(hwnd);
            DestroyWindow(hwnd);
            assert_ne!(visible, 0, "Restore depended on an egui repaint");
        }
    }
}
