//! Native tray lifetime and events stay on eframe's Windows message-loop thread.
use crate::config::log;
use eframe::egui::{Context, IconData, ViewportCommand};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

pub fn window_icon() -> IconData {
    IconData {
        rgba: include_bytes!("../assets/stethoscope.rgba").to_vec(),
        width: 64,
        height: 64,
    }
}

pub struct Tray {
    _icon: TrayIcon,
    quitting: Arc<AtomicBool>,
}
impl Tray {
    pub fn new(ctx: &Context) -> Result<Self, Box<dyn std::error::Error>> {
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
                restore(&menu_ctx);
            } else if event.id == hide_id {
                menu_ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                log("Window hidden from tray menu; collection continues");
            } else if event.id == quit_id {
                flag.store(true, Ordering::Relaxed);
                // Wake a hidden window too; on close, the app's ordinary Drop joins collection.
                menu_ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                menu_ctx.send_viewport_cmd(ViewportCommand::Close);
                log("Quit requested from tray menu");
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
                restore(&click_ctx);
            }
        }));
        log("Native stethoscope tray icon registered");
        Ok(Self {
            _icon: icon,
            quitting,
        })
    }
    pub fn quitting(&self) -> bool {
        self.quitting.load(Ordering::Relaxed)
    }
}
fn restore(ctx: &Context) {
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
    ctx.request_repaint();
    log("Window restored from tray");
}
impl Drop for Tray {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
        // TrayIcon's Drop calls Shell_NotifyIcon(NIM_DELETE).
        log("Removing native tray icon");
    }
}
