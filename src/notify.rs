use std::process::{Command, Stdio};

const TITLE: &str = "Pomodoro";

fn spawn(command: &str, args: &[&str]) -> bool {
    match Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

pub fn banner(message: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification {} with title {}",
            applescript_string(message),
            applescript_string(TITLE),
        );
        spawn("osascript", &["-e", &script]);
    }

    #[cfg(target_os = "linux")]
    {
        spawn(
            "notify-send",
            &[
                "--app-name=Pomodoro",
                "--icon=alarm-symbolic",
                TITLE,
                message,
            ],
        );
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
             $n = New-Object System.Windows.Forms.NotifyIcon; \
             $n.Icon = [System.Drawing.SystemIcons]::Information; \
             $n.BalloonTipTitle = '{}'; $n.BalloonTipText = '{}'; \
             $n.Visible = $true; $n.ShowBalloonTip(5000); Start-Sleep -Seconds 6",
            TITLE,
            message.replace('\'', "''"),
        );
        spawn(
            "powershell",
            &["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script],
        );
    }

    let _ = message;
}

pub fn chime() {
    #[cfg(target_os = "macos")]
    {
        spawn("afplay", &["/System/Library/Sounds/Glass.aiff"]);
    }

    #[cfg(target_os = "linux")]
    {
        if spawn("canberra-gtk-play", &["-i", "complete"]) {
            return;
        }
        const CANDIDATES: [&str; 3] = [
            "/usr/share/sounds/freedesktop/stereo/complete.oga",
            "/usr/share/sounds/freedesktop/stereo/bell.oga",
            "/usr/share/sounds/freedesktop/stereo/message.oga",
        ];
        for path in CANDIDATES {
            if std::path::Path::new(path).exists() && spawn("paplay", &[path]) {
                return;
            }
        }
        print!("\x07");
    }

    #[cfg(target_os = "windows")]
    {
        spawn(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "[console]::beep(880,220); [console]::beep(1320,260)",
            ],
        );
    }
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
