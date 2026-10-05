//! The tray icon: left click opens the window, the menu toggles auto-accept and quits.

use tauri::{
    AppHandle, Manager, Runtime,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use winer_core::{
    Service,
    settings::{Language, Settings},
};

use crate::window;

/// The mark without the app icon's plate (a tray composites a plate as a hard-edged block), drawn
/// natively for each display scale by `scripts/brand/icons.py`: Windows draws a tray icon 16px wide
/// at 100%, 20 at 125%, 24 at 150%, and resampling one size into another blurs it.
const ICONS: [(u32, &[u8]); 6] = [
    (16, include_bytes!("../icons/tray-16.rgba")),
    (20, include_bytes!("../icons/tray-20.rgba")),
    (24, include_bytes!("../icons/tray-24.rgba")),
    (32, include_bytes!("../icons/tray-32.rgba")),
    (40, include_bytes!("../icons/tray-40.rgba")),
    (48, include_bytes!("../icons/tray-48.rgba")),
];

/// The glyph for a display at `scale`: the smallest drawn at least `16 × scale` wide.
fn icon_for(scale: f64) -> Image<'static> {
    let wanted = (16.0 * scale).round() as u32;
    let (size, rgba) = ICONS
        .iter()
        .find(|(size, _)| *size >= wanted)
        .unwrap_or(&ICONS[ICONS.len() - 1]);
    Image::new(rgba, *size, *size)
}

struct Items<R: Runtime> {
    show: MenuItem<R>,
    accept: CheckMenuItem<R>,
    quit: MenuItem<R>,
}

fn labels(language: Language) -> [&'static str; 3] {
    match language {
        Language::ZhCn => ["打开 winer", "自动接受对局", "退出"],
        Language::En => ["Open winer", "Auto-accept matches", "Quit"],
    }
}

pub(crate) fn create<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> tauri::Result<()> {
    let [show, accept, quit] = labels(settings.general.language);
    let items = Items {
        show: MenuItem::with_id(app, "show", show, true, None::<&str>)?,
        accept: CheckMenuItem::with_id(
            app,
            "accept",
            accept,
            true,
            settings.automation.accept.enabled,
            None::<&str>,
        )?,
        quit: MenuItem::with_id(app, "quit", quit, true, None::<&str>)?,
    };
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &items.show,
            &separator,
            &items.accept,
            &separator,
            &items.quit,
        ],
    )?;
    // The tray sits on the primary display's taskbar.
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |monitor| monitor.scale_factor());
    TrayIconBuilder::with_id("main")
        .icon(icon_for(scale))
        .tooltip("winer")
        .menu(&menu)
        // Right click owns the menu, left click owns the window: both are needed.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => window::show(app),
            "accept" => toggle_accept(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if opens_window(&event) {
                window::show(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(items);
    Ok(())
}

/// Left button down only: Tauri reports both edges of a click, and a double click starts with one.
fn opens_window(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Down,
            ..
        }
    )
}

fn toggle_accept<R: Runtime>(app: &AppHandle<R>) {
    let Some(service) = app.try_state::<Service>() else {
        return;
    };
    let mut settings = service.settings();
    settings.automation.accept.enabled = !settings.automation.accept.enabled;
    if let Err(error) = service.set_settings(settings) {
        tracing::warn!(%error, "auto-accept toggle not saved");
    }
}

/// Keeps the menu in step with settings changed anywhere else.
pub(crate) fn sync<R: Runtime>(app: &AppHandle<R>, settings: &Settings) {
    let Some(items) = app.try_state::<Items<R>>() else {
        return;
    };
    let [show, accept, quit] = labels(settings.general.language);
    let _ = items.show.set_text(show);
    let _ = items.accept.set_text(accept);
    let _ = items.accept.set_checked(settings.automation.accept.enabled);
    let _ = items.quit.set_text(quit);
}

#[cfg(test)]
mod tests {
    use tauri::{
        PhysicalPosition, PhysicalSize, Position, Rect, Size,
        tray::{MouseButton, MouseButtonState, TrayIconEvent, TrayIconId},
    };

    use super::{ICONS, icon_for, opens_window};

    fn click(button: MouseButton, button_state: MouseButtonState) -> TrayIconEvent {
        TrayIconEvent::Click {
            id: TrayIconId::new("main"),
            position: PhysicalPosition::new(0.0, 0.0),
            rect: Rect {
                position: Position::Physical(PhysicalPosition::new(0, 0)),
                size: Size::Physical(PhysicalSize::new(16, 16)),
            },
            button,
            button_state,
        }
    }

    #[test]
    fn each_display_scale_gets_a_glyph_drawn_for_it() {
        for (size, rgba) in ICONS {
            assert_eq!(rgba.len(), (size * size * 4) as usize, "{size}px is RGBA");
        }
        let width = |scale| icon_for(scale).width();
        assert_eq!(
            [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0].map(width),
            [16, 20, 24, 32, 32, 40, 48, 48]
        );
    }

    #[test]
    fn one_left_click_opens_the_window_once() {
        let opened = [MouseButtonState::Down, MouseButtonState::Up]
            .into_iter()
            .filter(|&state| opens_window(&click(MouseButton::Left, state)))
            .count();
        assert_eq!(opened, 1);
        assert!(!opens_window(&click(
            MouseButton::Right,
            MouseButtonState::Down
        )));
        assert!(!opens_window(&click(
            MouseButton::Middle,
            MouseButtonState::Down
        )));
    }
}
