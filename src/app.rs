use crate::anim::{self, GENTLE, MotionSet, SMOOTH, SNAPPY, SWIFT};
use crate::config::{self, Settings, Stats};
use crate::icons;
use crate::notify;
use crate::paint::{PlayPause, Ring};
use crate::session::Session;
use crate::theme;
use gpui_kit::*;
use std::collections::HashSet;
use std::time::{Duration, Instant};

gpui_kit::actions!(
    pomodoro,
    [
        TogglePlay,
        ResetTimer,
        SkipSession,
        ToggleSettings,
        DismissSettings,
        SelectFocus,
        SelectShortBreak,
        SelectLongBreak,
        Quit,
    ]
);

const CONTEXT: &str = "Pomodoro";

const DOT_KEYS: [&str; 12] = [
    "dot0", "dot1", "dot2", "dot3", "dot4", "dot5", "dot6", "dot7", "dot8", "dot9", "dot10",
    "dot11",
];

const MODE_KEYS: [&str; 3] = ["mode0", "mode1", "mode2"];

const SWITCH_WIDTH: f32 = 300.0;
const SWITCH_PAD: f32 = 4.0;
const SEGMENT_WIDTH: f32 = (SWITCH_WIDTH - SWITCH_PAD * 2.0) / 3.0;
const RING_SIZE: f32 = 264.0;
const SHEET_TRAVEL: f32 = 520.0;
const BURST_SECS: f32 = 0.9;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlay, Some(CONTEXT)),
        KeyBinding::new("enter", TogglePlay, Some(CONTEXT)),
        KeyBinding::new("k", TogglePlay, Some(CONTEXT)),
        KeyBinding::new("r", ResetTimer, Some(CONTEXT)),
        KeyBinding::new("s", SkipSession, Some(CONTEXT)),
        KeyBinding::new("n", SkipSession, Some(CONTEXT)),
        KeyBinding::new("1", SelectFocus, Some(CONTEXT)),
        KeyBinding::new("2", SelectShortBreak, Some(CONTEXT)),
        KeyBinding::new("3", SelectLongBreak, Some(CONTEXT)),
        KeyBinding::new(",", ToggleSettings, Some(CONTEXT)),
        KeyBinding::new("escape", DismissSettings, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-q", Quit, None),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
}

pub struct Pomodoro {
    focus: FocusHandle,

    settings: Settings,
    stats: Stats,

    session: Session,
    duration: Duration,
    remaining: Duration,
    running: bool,
    cycle: u32,

    ticking: bool,
    last_tick: Option<Instant>,
    last_frame: Instant,

    motions: MotionSet,
    hover: MotionSet,
    press: MotionSet,
    hovered: HashSet<&'static str>,
    pressed: Option<&'static str>,

    accent_from: Hsla,
    burst_at: Option<Instant>,
    breath: f32,
    settings_open: bool,
    focus_claimed: bool,
    reduce_motion: bool,
    title: String,
}

impl Pomodoro {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (settings, stats) = config::load();
        let session = Session::Work;
        let duration = settings.duration(session);

        Self {
            focus: cx.focus_handle(),
            settings,
            stats,
            session,
            duration,
            remaining: duration,
            running: false,
            cycle: 0,
            ticking: false,
            last_tick: None,
            last_frame: Instant::now(),
            motions: MotionSet::new(),
            hover: MotionSet::new(),
            press: MotionSet::new(),
            hovered: HashSet::new(),
            pressed: None,
            accent_from: session.accent(),
            burst_at: None,
            breath: 0.0,
            settings_open: false,
            focus_claimed: false,
            reduce_motion: false,
            title: String::new(),
        }
    }

    // Timer

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.running {
            self.running = false;
            self.last_tick = None;
        } else {
            if self.remaining.is_zero() {
                self.remaining = self.duration;
            }
            self.running = true;
            self.last_tick = Some(Instant::now());
            self.start_ticking(cx);
        }
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.running = false;
        self.last_tick = None;
        self.duration = self.settings.duration(self.session);
        self.remaining = self.duration;
        cx.notify();
    }

    fn select(&mut self, session: Session, keep_running: bool, cx: &mut Context<Self>) {
        self.enter(session, keep_running && self.running);
        if self.running {
            self.start_ticking(cx);
        }
        cx.notify();
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        let next = self.next_session();
        let carry_on = self.running && self.auto_starts(next);
        self.enter(next, carry_on);
        if self.running {
            self.start_ticking(cx);
        }
        cx.notify();
    }

    fn next_session(&self) -> Session {
        match self.session {
            Session::Work => {
                if self.cycle >= self.settings.long_interval {
                    Session::LongBreak
                } else {
                    Session::ShortBreak
                }
            }
            _ => Session::Work,
        }
    }

    fn auto_starts(&self, next: Session) -> bool {
        if next.is_break() {
            self.settings.auto_start_breaks
        } else {
            self.settings.auto_start_focus
        }
    }

    fn enter(&mut self, session: Session, running: bool) {
        self.accent_from = self.accent();
        self.motions.jump("accent", 0.0);
        self.session = session;
        self.duration = self.settings.duration(session);
        self.remaining = self.duration;
        self.running = running;
        self.last_tick = running.then(Instant::now);
    }

    fn complete(&mut self) {
        let finished = self.session;
        match finished {
            Session::Work => {
                self.cycle += 1;
                self.stats.record_focus(self.duration);
            }
            Session::LongBreak => self.cycle = 0,
            Session::ShortBreak => {}
        }

        if self.settings.notifications {
            notify::banner(finished.completion_message());
        }
        if self.settings.sound {
            notify::chime();
        }
        self.burst_at = Some(Instant::now());

        let next = self.next_session();
        let auto_start = self.auto_starts(next);

        self.enter(next, auto_start);
        self.persist();
    }

    fn start_ticking(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let Ok(keep_going) = this.update(cx, |app, cx| app.tick(cx)) else {
                    break;
                };
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.running {
            self.ticking = false;
            self.last_tick = None;
            return false;
        }

        let now = Instant::now();
        let elapsed = self
            .last_tick
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        self.last_tick = Some(now);

        if self.remaining > elapsed {
            self.remaining -= elapsed;
        } else {
            self.remaining = Duration::ZERO;
            self.complete();
            if !self.running {
                self.ticking = false;
                self.last_tick = None;
            }
        }

        cx.notify();
        self.running
    }

    fn persist(&self) {
        config::save(&self.settings, &self.stats);
    }

    fn update_settings(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings);
        self.settings.long_interval = self.settings.long_interval.clamp(2, 12);
        self.cycle = self.cycle.min(self.settings.long_interval);

        let duration = self.settings.duration(self.session);
        if self.running {
            let elapsed = self.duration.saturating_sub(self.remaining);
            self.remaining = duration.saturating_sub(elapsed).max(Duration::from_secs(1));
        } else {
            self.remaining = duration;
        }
        self.duration = duration;

        self.persist();
        cx.notify();
    }

    fn accent(&self) -> Hsla {
        theme::mix(
            self.accent_from,
            self.session.accent(),
            self.motions.value("accent"),
        )
    }

    fn burst(&self) -> f32 {
        if self.reduce_motion {
            return 0.0;
        }

        self.burst_at
            .map(|at| {
                let elapsed = at.elapsed().as_secs_f32();
                if elapsed >= BURST_SECS {
                    0.0
                } else {
                    1.0 - anim::ease_out_cubic(elapsed / BURST_SECS)
                }
            })
            .unwrap_or(0.0)
    }

    fn clock(&self) -> (u64, u64) {
        // Ceil so that "00:01" stays on screen for its whole final second.
        let total = self.remaining.as_secs_f32().ceil() as u64;
        (total / 60, total % 60)
    }

    fn is_hovered(&self, key: &'static str) -> bool {
        self.hovered.contains(key)
    }

    fn set_hover(&mut self, key: &'static str, hovered: bool) {
        if hovered {
            self.hovered.insert(key);
        } else {
            self.hovered.remove(key);
            if self.pressed == Some(key) {
                self.pressed = None;
            }
        }
    }

    fn sync_title(&mut self, window: &mut Window) {
        let (minutes, seconds) = self.clock();
        let title = format!(
            "{minutes:02}:{seconds:02} · {} — Pomodoro",
            self.session.label()
        );
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
            if self.stats.roll_over() {
                self.persist();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn icon_button(
        &mut self,
        key: &'static str,
        glyph: &'static [u8],
        slot: f32,
        icon: f32,
        tint: Hsla,
        active_tint: Hsla,
        cx: &mut Context<Self>,
        on_press: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let hovered = self.is_hovered(key);
        let pressed = self.pressed == Some(key);
        let h = self.hover.to(key, SWIFT, if hovered { 1.0 } else { 0.0 });
        let p = self.press.to(key, SWIFT, if pressed { 1.0 } else { 0.0 });

        let disc = slot - 8.0 + 4.0 * h - 5.0 * p;

        div()
            .id(key)
            .size(px(slot))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .size(px(disc))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme::surface().alpha(h * 0.9))
                    .border_1()
                    .border_color(theme::border().alpha(h))
                    .child(
                        svg()
                            .size(px(icon))
                            .text_color(theme::mix(tint, active_tint, h))
                            .data(glyph),
                    ),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hover(key, *hovered);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.pressed = Some(key);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.pressed == Some(key) {
                        this.pressed = None;
                    }
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.pressed = None;
                on_press(this, window, cx);
                cx.notify();
            }))
    }

    fn header(&mut self, accent: Hsla, cx: &mut Context<Self>) -> Div {
        let glow = self.motions.value("running");

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .size(px(7.0))
                            .rounded_full()
                            .bg(accent.alpha(0.5 + 0.5 * glow)),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::muted())
                            .child(spaced("pomodoro")),
                    ),
            )
            .child(self.icon_button(
                "settings",
                icons::SETTINGS,
                34.0,
                16.0,
                theme::faint(),
                theme::text(),
                cx,
                |this, _, cx| {
                    this.settings_open = !this.settings_open;
                    cx.notify();
                },
            ))
    }

    fn mode_switch(&mut self, accent: Hsla, cx: &mut Context<Self>) -> Div {
        let position = self.motions.to("mode", SNAPPY, self.session.index() as f32);

        let mut row = div()
            .relative()
            .w(px(SWITCH_WIDTH))
            .h(px(40.0))
            .rounded_full()
            .bg(theme::surface().alpha(0.7))
            .border_1()
            .border_color(theme::border().alpha(0.7))
            .child(
                div()
                    .absolute()
                    .top(px(SWITCH_PAD))
                    .left(px(SWITCH_PAD + position * SEGMENT_WIDTH))
                    .w(px(SEGMENT_WIDTH))
                    .h(px(30.0))
                    .rounded_full()
                    .bg(accent.alpha(0.16))
                    .border_1()
                    .border_color(accent.alpha(0.32)),
            );

        for (index, session) in Session::ALL.into_iter().enumerate() {
            let key = MODE_KEYS[index];
            let selected = 1.0 - (position - index as f32).abs().clamp(0.0, 1.0);
            let hovered = self.is_hovered(key);
            let h = self.hover.to(key, SWIFT, if hovered { 1.0 } else { 0.0 });
            let idle = theme::mix(theme::muted(), theme::text(), h * 0.6);

            row = row.child(
                div()
                    .id(key)
                    .absolute()
                    .top(px(SWITCH_PAD))
                    .left(px(SWITCH_PAD + index as f32 * SEGMENT_WIDTH))
                    .w(px(SEGMENT_WIDTH))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .text_color(theme::mix(idle, accent, selected))
                    .child(session.short_label())
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.set_hover(key, *hovered);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.session != session {
                            this.select(session, false, cx);
                        }
                    })),
            );
        }

        row
    }

    fn dial(&mut self, accent: Hsla, progress: f32, glow: f32) -> Div {
        let (minutes, seconds) = self.clock();
        let text = format!("{minutes:02}:{seconds:02}");
        let burst = self.burst();

        let rounds = self.settings.long_interval;
        let round = (self.cycle % rounds.max(1)) + 1;
        let caption = if self.session.is_break() {
            "up next · focus".to_string()
        } else {
            format!("round {round} of {rounds}")
        };

        let mut digits = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .text_size(px(56.0))
            .line_height(px(64.0))
            .text_color(theme::text());

        for (index, character) in text.chars().enumerate() {
            digits = if character == ':' {
                digits.child(
                    div()
                        .w(px(20.0))
                        .text_center()
                        .text_color(theme::text().alpha(0.35 + 0.35 * glow))
                        .child(":"),
                )
            } else {
                digits.child(digit(index, character))
            };
        }

        div()
            .relative()
            .size(px(RING_SIZE))
            .flex_none()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(150.0)).rounded_full().shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), accent.alpha(0.16 * glow))
                            .blur_radius(px(90.0))
                            .spread_radius(px(24.0)),
                    ])),
            )
            .child(
                div().absolute().inset_0().child(
                    Ring {
                        progress,
                        accent,
                        track: theme::track(),
                        thickness: 9.0,
                        glow,
                        burst,
                    }
                    .render(),
                ),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(accent.alpha(0.9))
                            .child(spaced(self.session.label())),
                    )
                    .child(digits)
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::faint())
                            .child(spaced(&caption)),
                    ),
            )
    }

    fn dots(&mut self, accent: Hsla) -> Div {
        let rounds = self.settings.long_interval.clamp(2, 12) as usize;
        let filled = self.cycle.min(self.settings.long_interval) as usize;

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.0));

        for (index, key) in DOT_KEYS.iter().copied().take(rounds).enumerate() {
            let target = if index < filled {
                1.0
            } else if index == filled && !self.session.is_break() {
                0.3
            } else {
                0.0
            };
            let t = self.motions.to(key, SNAPPY, target);
            let size = 5.0 + 3.0 * t;

            row = row.child(
                div()
                    .size(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(size)).rounded_full().bg(theme::mix(
                        theme::hex(0x2c313a),
                        accent,
                        t,
                    ))),
            );
        }

        row
    }

    fn controls(&mut self, accent: Hsla, running: f32, glow: f32, cx: &mut Context<Self>) -> Div {
        let hovered = self.is_hovered("play");
        let pressed = self.pressed == Some("play");
        let h = self
            .hover
            .to("play", SWIFT, if hovered { 1.0 } else { 0.0 });
        let p = self
            .press
            .to("play", SWIFT, if pressed { 1.0 } else { 0.0 });
        let disc = 86.0 + 4.0 * h - 6.0 * p;

        let play = div()
            .id("play")
            .size(px(100.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .size(px(disc))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(accent.alpha(0.12 + 0.06 * h))
                    .border_1()
                    .border_color(accent.alpha(0.34 + 0.24 * h))
                    .shadow(vec![
                        BoxShadow::new(
                            px(0.0),
                            px(0.0),
                            accent.alpha(0.10 + 0.16 * glow + 0.08 * h),
                        )
                        .blur_radius(px(38.0))
                        .spread_radius(px(2.0)),
                    ])
                    .child(
                        div().size(px(30.0)).child(
                            PlayPause {
                                t: running,
                                color: accent,
                                size: 26.0,
                            }
                            .render(),
                        ),
                    ),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.set_hover("play", *hovered);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.pressed = Some("play");
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.pressed = None;
                    cx.notify();
                }),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.pressed = None;
                this.toggle(cx);
            }));

        let reset = self.icon_button(
            "reset",
            icons::RESET,
            48.0,
            17.0,
            theme::faint(),
            theme::text(),
            cx,
            |this, _, cx| this.reset(cx),
        );

        let skip = self.icon_button(
            "skip",
            icons::SKIP,
            48.0,
            17.0,
            theme::faint(),
            theme::text(),
            cx,
            |this, _, cx| this.skip(cx),
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(14.0))
            .child(reset)
            .child(play)
            .child(skip)
    }

    fn footer(&self) -> Div {
        let summary = if self.stats.today_sessions == 0 {
            "no sessions yet today".to_string()
        } else {
            format!(
                "{} today · {} focused",
                self.stats.today_sessions,
                humanise(self.stats.today_focus_secs)
            )
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::muted())
                    .child(summary),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::faint())
                    .child("space play · r reset · s skip · , settings"),
            )
    }

    // Settings sheet

    #[allow(clippy::too_many_arguments)]
    fn stepper(
        &mut self,
        label: &'static str,
        value: String,
        minus: &'static str,
        plus: &'static str,
        accent: Hsla,
        cx: &mut Context<Self>,
        on_minus: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        on_plus: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Div {
        let decrement = self.icon_button(
            minus,
            icons::MINUS,
            30.0,
            13.0,
            theme::muted(),
            theme::text(),
            cx,
            on_minus,
        );
        let increment = self.icon_button(
            plus,
            icons::PLUS,
            30.0,
            13.0,
            theme::muted(),
            theme::text(),
            cx,
            on_plus,
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(32.0))
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(theme::muted())
                    .child(label),
            )
            .child(decrement)
            .child(
                div()
                    .w(px(58.0))
                    .text_center()
                    .text_size(px(13.0))
                    .text_color(accent)
                    .child(animated_value(label, value)),
            )
            .child(increment)
    }

    fn switch(
        &mut self,
        key: &'static str,
        label: &'static str,
        on: bool,
        accent: Hsla,
        cx: &mut Context<Self>,
        on_press: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let t = self.motions.to(key, SNAPPY, if on { 1.0 } else { 0.0 });
        let hovered = self.is_hovered(key);
        let h = self.hover.to(key, SWIFT, if hovered { 1.0 } else { 0.0 });

        div()
            .id(key)
            .flex()
            .flex_row()
            .items_center()
            .h(px(30.0))
            .cursor_pointer()
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(theme::mix(theme::muted(), theme::text(), h))
                    .child(label),
            )
            .child(
                div()
                    .relative()
                    .w(px(40.0))
                    .h(px(23.0))
                    .rounded_full()
                    .bg(theme::mix(theme::track(), accent.alpha(0.85), t))
                    .border_1()
                    .border_color(theme::mix(theme::border(), accent, t).alpha(0.7 + 0.3 * h))
                    .child(
                        div()
                            .absolute()
                            .top(px(3.0))
                            .left(px(3.0 + 17.0 * t))
                            .size(px(15.0))
                            .rounded_full()
                            .bg(theme::mix(theme::muted(), theme::text(), t)),
                    ),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hover(key, *hovered);
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                on_press(this, window, cx);
                cx.notify();
            }))
    }

    fn section(title: &'static str) -> Div {
        div()
            .pt(px(8.0))
            .pb(px(2.0))
            .text_size(px(9.0))
            .text_color(theme::muted().alpha(0.65))
            .child(spaced(title))
    }

    fn settings_sheet(&mut self, t: f32, accent: Hsla, cx: &mut Context<Self>) -> Stateful<Div> {
        let settings = self.settings;

        let close = self.icon_button(
            "sheet-close",
            icons::CLOSE,
            32.0,
            14.0,
            theme::muted(),
            theme::text(),
            cx,
            |this, _, cx| {
                this.settings_open = false;
                cx.notify();
            },
        );

        let focus_row = self.stepper(
            "Focus",
            format!("{} min", settings.work_mins),
            "focus-minus",
            "focus-plus",
            accent,
            cx,
            |this, _, cx| {
                this.update_settings(cx, |s| {
                    s.set_minutes(Session::Work, s.work_mins.saturating_sub(5).max(1))
                })
            },
            |this, _, cx| {
                this.update_settings(cx, |s| s.set_minutes(Session::Work, s.work_mins + 5))
            },
        );

        let short_row = self.stepper(
            "Short break",
            format!("{} min", settings.short_mins),
            "short-minus",
            "short-plus",
            accent,
            cx,
            |this, _, cx| {
                this.update_settings(cx, |s| {
                    s.set_minutes(Session::ShortBreak, s.short_mins.saturating_sub(1).max(1))
                })
            },
            |this, _, cx| {
                this.update_settings(cx, |s| s.set_minutes(Session::ShortBreak, s.short_mins + 1))
            },
        );

        let long_row = self.stepper(
            "Long break",
            format!("{} min", settings.long_mins),
            "long-minus",
            "long-plus",
            accent,
            cx,
            |this, _, cx| {
                this.update_settings(cx, |s| {
                    s.set_minutes(Session::LongBreak, s.long_mins.saturating_sub(5).max(1))
                })
            },
            |this, _, cx| {
                this.update_settings(cx, |s| s.set_minutes(Session::LongBreak, s.long_mins + 5))
            },
        );

        let rounds_row = self.stepper(
            "Rounds per cycle",
            format!("{}", settings.long_interval),
            "rounds-minus",
            "rounds-plus",
            accent,
            cx,
            |this, _, cx| {
                this.update_settings(cx, |s| {
                    s.long_interval = s.long_interval.saturating_sub(1).max(2)
                })
            },
            |this, _, cx| this.update_settings(cx, |s| s.long_interval += 1),
        );

        let auto_breaks = self.switch(
            "auto-breaks",
            "Start breaks automatically",
            settings.auto_start_breaks,
            accent,
            cx,
            |this, _, cx| this.update_settings(cx, |s| s.auto_start_breaks = !s.auto_start_breaks),
        );

        let auto_focus = self.switch(
            "auto-focus",
            "Start focus automatically",
            settings.auto_start_focus,
            accent,
            cx,
            |this, _, cx| this.update_settings(cx, |s| s.auto_start_focus = !s.auto_start_focus),
        );

        let notifications = self.switch(
            "notifications",
            "Desktop notifications",
            settings.notifications,
            accent,
            cx,
            |this, _, cx| this.update_settings(cx, |s| s.notifications = !s.notifications),
        );

        let sound = self.switch(
            "sound",
            "Completion chime",
            settings.sound,
            accent,
            cx,
            |this, _, cx| this.update_settings(cx, |s| s.sound = !s.sound),
        );

        let totals = format!(
            "{} sessions · {} focused all time",
            self.stats.total_sessions,
            humanise(self.stats.total_focus_secs)
        );

        let clear_hovered = self.is_hovered("clear-stats");
        let clear_h = self
            .hover
            .to("clear-stats", SWIFT, if clear_hovered { 1.0 } else { 0.0 });

        let card = div()
            .id("sheet")
            .occlude()
            .relative()
            .bottom(px(-SHEET_TRAVEL * (1.0 - t)))
            .w_full()
            .max_w(px(430.0))
            .m(px(14.0))
            .p(px(18.0))
            .rounded(px(20.0))
            .bg(theme::surface())
            .border_1()
            .border_color(theme::border())
            .shadow(vec![
                BoxShadow::new(px(0.0), px(18.0), theme::hex(0x000000).alpha(0.55))
                    .blur_radius(px(48.0)),
            ])
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::text())
                            .child("Settings"),
                    )
                    .child(close),
            )
            .child(Self::section("durations"))
            .child(focus_row)
            .child(short_row)
            .child(long_row)
            .child(rounds_row)
            .child(Self::section("automation"))
            .child(auto_breaks)
            .child(auto_focus)
            .child(Self::section("alerts"))
            .child(notifications)
            .child(sound)
            .child(Self::section("statistics"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(30.0))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.0))
                            .text_color(theme::muted())
                            .child(totals),
                    )
                    .child(
                        div()
                            .id("clear-stats")
                            .px(px(10.0))
                            .py(px(5.0))
                            .rounded(px(8.0))
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .bg(theme::track().alpha(clear_h))
                            .text_color(theme::mix(theme::faint(), accent, clear_h))
                            .child("Reset")
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.set_hover("clear-stats", *hovered);
                                cx.notify();
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stats.clear();
                                this.persist();
                                cx.notify();
                            })),
                    ),
            );

        div()
            .id("scrim")
            .occlude()
            .absolute()
            .inset_0()
            .bg(theme::bg().alpha(0.78 * t))
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .child(card)
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings_open = false;
                cx.notify();
            }))
    }

    // Frame

    fn body(&mut self, cx: &mut Context<Self>) -> Div {
        let accent_t = self.motions.to("accent", SMOOTH, 1.0);
        let accent = theme::mix(self.accent_from, self.session.accent(), accent_t);

        let running = self
            .motions
            .to("running", SMOOTH, if self.running { 1.0 } else { 0.0 });

        let target_progress = if self.duration.is_zero() {
            0.0
        } else {
            1.0 - self.remaining.as_secs_f32() / self.duration.as_secs_f32()
        };
        let progress = self
            .motions
            .to("progress", SMOOTH, target_progress.clamp(0.0, 1.0));

        let breath = if self.reduce_motion {
            0.5
        } else {
            (self.breath * 1.6).sin() * 0.5 + 0.5
        };
        let glow = running * (0.35 + 0.65 * breath);

        let sheet = self
            .motions
            .to("sheet", GENTLE, if self.settings_open { 1.0 } else { 0.0 });

        let header = self.header(accent, cx);
        let modes = self.mode_switch(accent, cx);
        let dial = self.dial(accent, progress, glow);
        let dots = self.dots(accent);
        let controls = self.controls(accent, running, glow, cx);
        let footer = self.footer();

        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .px(px(22.0))
            .pt(px(18.0))
            .pb(px(22.0))
            .gap(px(14.0))
            .child(div().w_full().child(header))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(18.0))
                    .child(modes)
                    .child(dial)
                    .child(dots)
                    .child(controls),
            )
            .child(footer);

        let mut root = div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .bg(theme::bg())
            .text_color(theme::text())
            .font_family("Maple Mono NF")
            .child(content);

        if sheet > 0.001 {
            let overlay = self.settings_sheet(sheet, accent, cx);
            root = root.child(overlay);
        }

        root.on_action(cx.listener(|this, _: &TogglePlay, _, cx| this.toggle(cx)))
            .on_action(cx.listener(|this, _: &ResetTimer, _, cx| this.reset(cx)))
            .on_action(cx.listener(|this, _: &SkipSession, _, cx| this.skip(cx)))
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.settings_open = !this.settings_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &DismissSettings, _, cx| {
                if this.settings_open {
                    this.settings_open = false;
                    cx.notify();
                }
            }))
            .on_action(
                cx.listener(|this, _: &SelectFocus, _, cx| this.select(Session::Work, false, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectShortBreak, _, cx| {
                this.select(Session::ShortBreak, false, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectLongBreak, _, cx| {
                this.select(Session::LongBreak, false, cx)
            }))
    }
}

impl Render for Pomodoro {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        // Clamped so that waking from sleep does not fling every spring.
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        self.reduce_motion = cx.reduce_motion();
        self.motions.set_reduced(self.reduce_motion);
        self.hover.set_reduced(self.reduce_motion);
        self.press.set_reduced(self.reduce_motion);

        self.motions.step(dt);
        self.hover.step(dt);
        self.press.step(dt);
        if !self.reduce_motion && self.motions.value("running") > 0.005 {
            self.breath += dt;
        }

        if !self.focus_claimed {
            self.focus_claimed = true;
            window.focus(&self.focus, cx);
        }
        self.sync_title(window);

        let root = self.body(cx);

        let animating = self.motions.animating()
            || self.hover.animating()
            || self.press.animating()
            || self.burst() > 0.0;
        if animating && !self.running {
            window.request_animation_frame();
        }

        root
    }
}

impl Focusable for Pomodoro {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn digit(index: usize, character: char) -> impl IntoElement {
    let id = ElementId::Name(format!("digit-{index}-{character}").into());

    div()
        .relative()
        .w(px(36.0))
        .text_center()
        .child(character.to_string())
        .with_animation(
            id,
            Animation::new(Duration::from_millis(240)).with_easing(anim::ease_out_cubic),
            |element, t| element.opacity(0.35 + 0.65 * t).top(px(5.0 * (1.0 - t))),
        )
}

fn animated_value(key: &'static str, value: String) -> impl IntoElement {
    let id = ElementId::Name(format!("value-{key}-{value}").into());

    div().relative().child(value).with_animation(
        id,
        Animation::new(Duration::from_millis(220)).with_easing(anim::ease_out_cubic),
        |element, t| element.opacity(0.3 + 0.7 * t).top(px(4.0 * (1.0 - t))),
    )
}

fn spaced(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanise(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}
