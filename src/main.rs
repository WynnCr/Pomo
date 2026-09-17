use gpui_kit::component::Root;
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::*;
use std::borrow::Cow;
use std::time::Duration;
struct App {
    remaining: u64,
    running: bool,
    timer_started: bool,
}

impl App {
    fn toggle_timer(&mut self, cx: &mut Context<Self>) {
        self.running = !self.running;

        if self.running && !self.timer_started {
            self.timer_started = true;
            self.start_timer(cx);
        }
    }

    fn reset(&mut self) {
        self.remaining = 25 * 60;
        self.running = false;
    }

    fn start_timer(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;

                let should_continue = entity.update(cx, |app, cx| {
                    if !app.running {
                        return true;
                    }

                    if app.remaining > 0 {
                        app.remaining -= 1;
                        cx.notify();
                        true
                    } else {
                        app.running = false;
                        app.timer_started = false;
                        cx.notify();
                        false
                    }
                })?;

                if !should_continue {
                    break;
                }
            }

            Ok::<(), gpui_kit::private::anyhow::Error>(())
        })
        .detach();
    }
}

impl Render for App {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let minutes = self.remaining / 60;
        let seconds = self.remaining % 60;

        let timer_text = format!("{minutes:02}:{seconds:02}");

        let button_text = if self.running {
            "Pause"
        } else if self.remaining == 25 * 60 {
            "Start Timer"
        } else {
            "Resume"
        };

        div()
            .size_full()
            .bg(rgb(0x0f1012))
            .text_color(rgb(0xe8e9ea))
            .font_family("Maple Mono NF")
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(div().text_size(px(80.0)).child(timer_text))
                    .child(
                        Button::new("start")
                            .danger()
                            .large()
                            .label(button_text)
                            .w(px(180.0))
                            .h(px(50.0))
                            .rounded(px(15.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_timer(cx);
                            })),
                    )
                    .child(
                        Button::new("reset")
                            .label("Reset")
                            .w(px(180.0))
                            .h(px(45.0))
                            .rounded(px(15.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reset();
                                cx.notify();
                            })),
                    ),
            )
    }
}
fn main() {
    let app = gpui_kit::application();

    app.run(|cx| {
        gpui_kit::init(cx);
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../font/MapleMono-NF-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../font/MapleMono-NF-Bold.ttf").as_slice()),
            ])
            .expect("Failed to load font");

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(1200.0), px(760.0)),
                })),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| App {
                    remaining: 25 * 60,
                    running: false,
                    timer_started: false,
                });

                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open window");

        cx.activate(true);
    });
}
