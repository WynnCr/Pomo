use gpui_kit::{Hsla, rgb};

pub fn hex(value: u32) -> Hsla {
    Hsla::from(rgb(value))
}

pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    let mut delta = to.h - from.h;
    if delta > 0.5 {
        delta -= 1.0;
    } else if delta < -0.5 {
        delta += 1.0;
    }

    Hsla {
        h: (from.h + delta * t).rem_euclid(1.0),
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

pub fn bg() -> Hsla {
    hex(0x0a0b0d)
}

pub fn surface() -> Hsla {
    hex(0x141619)
}

pub fn border() -> Hsla {
    hex(0x23262d)
}

pub fn track() -> Hsla {
    hex(0x1b1e23)
}

pub fn text() -> Hsla {
    hex(0xeceef2)
}

pub fn muted() -> Hsla {
    hex(0x848b98)
}

pub fn faint() -> Hsla {
    hex(0x4b515c)
}
