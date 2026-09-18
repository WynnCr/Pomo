use crate::session::Session;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FILE_NAME: &str = "state.conf";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub work_mins: u32,
    pub short_mins: u32,
    pub long_mins: u32,
    pub long_interval: u32,
    pub auto_start_breaks: bool,
    pub auto_start_focus: bool,
    pub notifications: bool,
    pub sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_mins: 25,
            short_mins: 5,
            long_mins: 15,
            long_interval: 4,
            auto_start_breaks: true,
            auto_start_focus: false,
            notifications: true,
            sound: true,
        }
    }
}

impl Settings {
    pub fn minutes(&self, session: Session) -> u32 {
        match session {
            Session::Work => self.work_mins,
            Session::ShortBreak => self.short_mins,
            Session::LongBreak => self.long_mins,
        }
    }

    pub fn duration(&self, session: Session) -> Duration {
        Duration::from_secs(self.minutes(session).max(1) as u64 * 60)
    }

    /// Allowed range for each adjustable length, in minutes.
    pub fn bounds(session: Session) -> (u32, u32) {
        match session {
            Session::Work => (1, 180),
            Session::ShortBreak => (1, 60),
            Session::LongBreak => (1, 120),
        }
    }

    pub fn set_minutes(&mut self, session: Session, value: u32) {
        let (min, max) = Self::bounds(session);
        let value = value.clamp(min, max);
        match session {
            Session::Work => self.work_mins = value,
            Session::ShortBreak => self.short_mins = value,
            Session::LongBreak => self.long_mins = value,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    /// Whole days since the Unix epoch, used to roll the daily counters over.
    pub day: u64,
    pub today_sessions: u32,
    pub today_focus_secs: u64,
    pub total_sessions: u32,
    pub total_focus_secs: u64,
}

impl Stats {
    pub fn roll_over(&mut self) -> bool {
        let today = current_day();
        if self.day == today {
            return false;
        }
        self.day = today;
        self.today_sessions = 0;
        self.today_focus_secs = 0;
        true
    }

    pub fn record_focus(&mut self, elapsed: Duration) {
        self.roll_over();
        let secs = elapsed.as_secs();
        self.today_sessions += 1;
        self.today_focus_secs += secs;
        self.total_sessions += 1;
        self.total_focus_secs += secs;
    }

    pub fn clear(&mut self) {
        *self = Stats {
            day: current_day(),
            ..Default::default()
        };
    }
}

fn current_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(dir).join("pomo"));
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("pomo"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("pomo")
        })
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("pomo"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(FILE_NAME))
}

pub fn load() -> (Settings, Stats) {
    let mut settings = Settings::default();
    let mut stats = Stats::default();

    let Some(contents) = config_path().and_then(|path| fs::read_to_string(path).ok()) else {
        stats.day = current_day();
        return (settings, stats);
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        let number = |fallback: u64| value.parse::<u64>().unwrap_or(fallback);
        let flag = |fallback: bool| match value {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            _ => fallback,
        };

        match key {
            "work_mins" => settings.work_mins = number(settings.work_mins as u64) as u32,
            "short_mins" => settings.short_mins = number(settings.short_mins as u64) as u32,
            "long_mins" => settings.long_mins = number(settings.long_mins as u64) as u32,
            "long_interval" => {
                settings.long_interval = number(settings.long_interval as u64) as u32
            }
            "auto_start_breaks" => settings.auto_start_breaks = flag(settings.auto_start_breaks),
            "auto_start_focus" => settings.auto_start_focus = flag(settings.auto_start_focus),
            "notifications" => settings.notifications = flag(settings.notifications),
            "sound" => settings.sound = flag(settings.sound),
            "day" => stats.day = number(0),
            "today_sessions" => stats.today_sessions = number(0) as u32,
            "today_focus_secs" => stats.today_focus_secs = number(0),
            "total_sessions" => stats.total_sessions = number(0) as u32,
            "total_focus_secs" => stats.total_focus_secs = number(0),
            _ => {}
        }
    }

    settings.work_mins = settings.work_mins.clamp(1, 180);
    settings.short_mins = settings.short_mins.clamp(1, 60);
    settings.long_mins = settings.long_mins.clamp(1, 120);
    settings.long_interval = settings.long_interval.clamp(2, 12);
    stats.roll_over();

    (settings, stats)
}

pub fn save(settings: &Settings, stats: &Stats) {
    let Some(dir) = config_dir() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let body = format!(
        "# pomo state — edited automatically\n\
         work_mins={}\n\
         short_mins={}\n\
         long_mins={}\n\
         long_interval={}\n\
         auto_start_breaks={}\n\
         auto_start_focus={}\n\
         notifications={}\n\
         sound={}\n\
         day={}\n\
         today_sessions={}\n\
         today_focus_secs={}\n\
         total_sessions={}\n\
         total_focus_secs={}\n",
        settings.work_mins,
        settings.short_mins,
        settings.long_mins,
        settings.long_interval,
        settings.auto_start_breaks,
        settings.auto_start_focus,
        settings.notifications,
        settings.sound,
        stats.day,
        stats.today_sessions,
        stats.today_focus_secs,
        stats.total_sessions,
        stats.total_focus_secs,
    );

    let target = dir.join(FILE_NAME);
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    if fs::write(&temporary, body).is_ok() {
        let _ = fs::rename(&temporary, &target);
    }
}
