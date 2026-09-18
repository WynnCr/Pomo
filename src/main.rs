#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anim;
mod app;
mod config;
mod icons;
mod notify;
mod paint;
mod session;
mod theme;

use app::Pomodoro;
use gpui_kit::component::Root;
use gpui_kit::*;
use std::borrow::Cow;

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        app::init(cx);

        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../font/MapleMono-NF-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../font/MapleMono-NF-Bold.ttf").as_slice()),
            ])
            .expect("failed to load font");

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(440.0), px(700.0)),
                    cx,
                ))),
                window_min_size: Some(size(px(380.0), px(650.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Pomodoro".into()),
                    ..Default::default()
                }),
                // Matches the Flatpak app id so desktop shells can pair the
                // window with its .desktop entry.
                app_id: Some("com.amrit.Pomo".into()),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(Pomodoro::new);
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open window");

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        cx.activate(true);
    });
}
