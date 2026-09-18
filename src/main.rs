use gpui_kit::component::Root;
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Session {
    Work,
    ShortBreak,
    LongBreak,
}

impl Session {
    fn duration_secs(&self) -> u64 {
        match self {
            Session::Work => 25 * 60,
            Session::ShortBreak => 5 * 60,
            Session::LongBreak => 15 * 60,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Session::Work => "Focus",
            Session::ShortBreak => "Short Break",
            Session::LongBreak => "Long Break",
        }
    }

    fn accent(&self) -> u32 {
        match self {
            Session::Work => 0xe5484d,
            Session::ShortBreak => 0x4dabf7,
            Session::LongBreak => 0x69db7c,
        }
    }
}

struct App {
    session: Session,
    duration: Duration,
    remaining: Duration,
    running: bool,
    last_tick: Option<Instant>,
    completed_pomodoros: u32,
}

impl App {
    fn new() -> Self {
        let initial_session = Session::Work;
        Self {
            session: initial_session,
            duration: Duration::from_secs(initial_session.duration_secs()),
            remaining: Duration::from_secs(initial_session.duration_secs()),
            running: false,
            last_tick: None,
            completed_pomodoros: 0,
        }
    }

    fn toggle_timer(&mut self, cx: &mut Context<Self>) {
        self.running = !self.running;

        if self.running {
            self.last_tick = Some(Instant::now());
            self.start_timer(cx);
        } else {
            self.last_tick = None;
        }
    }

    fn reset(&mut self) {
        self.duration = Duration::from_secs(self.session.duration_secs());
        self.remaining = self.duration;
        self.running = false;
        self.last_tick = None;
    }

    fn advance_session(&mut self) {
        self.session = match self.session {
            Session::Work => {
                self.completed_pomodoros += 1;
                if self.completed_pomodoros.is_multiple_of(4) {
                    Session::LongBreak
                } else {
                    Session::ShortBreak
                }
            }
            Session::ShortBreak | Session::LongBreak => Session::Work,
        };
        self.reset(); // Re-initializes durations and stops timer
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        self.advance_session();
        cx.notify();
    }

    fn notify_session_end(&self) {
        let title = "Pomodoro Timer";
        let message = match self.session {
            Session::Work => "Focus session complete! Time for a break.",
            Session::ShortBreak | Session::LongBreak => "Break is over! Ready to focus?",
        };

        // Tried to reduce dependencies on unnecessary crates by using this approach
        #[cfg(target_os = "macos")]
        std::process::Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
                    message, title
                ),
            ])
            .spawn()
            .ok();

        #[cfg(target_os = "linux")]
        std::process::Command::new("notify-send")
            .args([title, message])
            .spawn()
            .ok();

        #[cfg(target_os = "windows")]
        std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{}', '{}')",
                    message, title
                ),
            ])
            .spawn()
            .ok();
    }

    fn start_timer(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| {
            loop {
                // Progress bar
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;

                let should_continue = entity.update(cx, |app, cx| {
                    if !app.running {
                        return false; // Kill the background task if paused, saves CPU.
                    }

                    let now = Instant::now();
                    let delta = if let Some(last) = app.last_tick {
                        now.duration_since(last)
                    } else {
                        Duration::ZERO
                    };
                    app.last_tick = Some(now);

                    if app.remaining > delta {
                        app.remaining -= delta;
                        cx.notify();
                        true
                    } else {
                        app.notify_session_end();

                        app.advance_session();
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
        // Use ceil so that 00:01 is shown until the very last millisecond
        let total_seconds = self.remaining.as_secs_f32().ceil() as u64;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let timer_text = format!("{minutes:02}:{seconds:02}");

        let accent = self.session.accent();
        let total_duration_secs = self.duration.as_secs_f32().max(1.0);
        let elapsed_fraction = 1.0 - (self.remaining.as_secs_f32() / total_duration_secs);

        let button_text = if self.running {
            "Pause"
        } else if self.remaining == self.duration {
            match self.session {
                Session::Work => "Start Focus",
                Session::ShortBreak | Session::LongBreak => "Start Break",
            }
        } else {
            "Resume"
        };

        let cycle_position = (self.completed_pomodoros % 4) as usize;
        let cycle_filled = if self.session == Session::LongBreak {
            4
        } else {
            cycle_position
        };

        let mut dots = div().flex().flex_row().gap_2();
        for i in 0..4 {
            let filled = i < cycle_filled;
            dots = dots.child(div().size(px(8.0)).rounded_full().bg(if filled {
                rgb(accent)
            } else {
                rgb(0x2a2c30)
            }));
        }

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
                    .gap_6()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(rgb(accent))
                            .child(self.session.label()),
                    )
                    .child(div().text_size(px(80.0)).child(timer_text))
                    // Progress bar
                    .child(
                        div()
                            .w(px(280.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(rgb(0x1e2024))
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(accent))
                                    .w(relative(elapsed_fraction.clamp(0.0, 1.0))),
                            ),
                    )
                    .child(dots)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_3()
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
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .child(
                                        Button::new("reset")
                                            .label("Reset")
                                            .w(px(85.0))
                                            .h(px(40.0))
                                            .rounded(px(12.0))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.reset();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("skip")
                                            .label("Skip")
                                            .w(px(85.0))
                                            .h(px(40.0))
                                            .rounded(px(12.0))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.skip(cx);
                                            })),
                                    ),
                            ),
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
                let view = cx.new(|_| App::new());
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open window");

        cx.activate(true);
    });
}
