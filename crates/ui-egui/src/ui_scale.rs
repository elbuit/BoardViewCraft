//! Preferences ▸ Interface size: how large the interface is drawn. `Auto` keeps the display's own
//! scale and adds the desktop's text scaling (KDE's general font size, GNOME's text scaling
//! factor, Windows' "Make text bigger"), read once at launch, so the labels match the other apps'.
//! A chosen size is absolute: "100%" is one screen pixel per point whatever scale the display
//! reports, so a system that reports 200% doesn't make "100%" draw at 200%.
//!
//! The scale is egui's zoom factor: `pixels_per_point` grows with it.

/// The smallest and largest interface sizes, as factors.
pub const MIN: f32 = 0.5;
pub const MAX: f32 = 3.0;

/// The sizes the Preferences window offers, in percent.
pub const PRESETS: [u16; 9] = [75, 80, 90, 100, 110, 125, 150, 175, 200];

/// A factor from settings or the system: finite and within [`MIN`]..=[`MAX`], or `None`.
pub fn valid(factor: f64) -> Option<f32> {
    let factor = factor as f32;
    (factor.is_finite() && (MIN..=MAX).contains(&factor)).then_some(factor)
}

/// egui's zoom factor on a display whose own scale is `native`: the system's text scale for
/// `Auto` (`None`, or a saved size out of range), or the chosen size with the display's scale
/// divided back out.
pub fn zoom(preference: Option<f32>, system: f32, native: f32) -> f32 {
    match preference.and_then(|size| valid(f64::from(size))) {
        None => valid(f64::from(system)).unwrap_or(1.0),
        Some(size) => size / usable(native),
    }
}

/// The display's scale, or 1.0 when egui doesn't know it (or reports nonsense).
fn usable(native: f32) -> f32 {
    if native.is_finite() && native > 0.0 { native } else { 1.0 }
}

/// The percentage a factor shows as.
pub fn percent(factor: f32) -> u32 {
    // Factors here went through `valid` (or are 1.0), so the cast can't overflow.
    (factor * 100.0).round().clamp(0.0, 1000.0) as u32
}

/// Draw at `factor` from the next frame on, when it differs from the factor last applied
/// (`applied`). egui's own Ctrl+/Ctrl−/Ctrl+0 interface zoom stays off: the preference alone
/// decides the interface size.
pub fn sync(ctx: &egui::Context, factor: f32, applied: &mut Option<f32>) {
    if applied.is_none() {
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
    }
    if *applied != Some(factor) {
        ctx.set_zoom_factor(factor);
        *applied = Some(factor);
    }
}

/// The desktop's text scaling factor, or 1.0. May spawn one short-lived process, so call it once
/// at launch.
pub fn system_text_scale() -> f32 {
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
    {
        unix::system_text_scale()
    }
    #[cfg(windows)]
    {
        windows::system_text_scale()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")))]
    {
        1.0
    }
}

/// KDE's default general font size, in points.
#[cfg_attr(not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")), allow(dead_code))]
const KDE_DEFAULT_POINTS: f64 = 10.0;

/// The text scale from a `kdeglobals` file: `[General]` `font=Family,SIZE,...` over KDE's default
/// 10 pt. `None` when the key is missing or unusable.
#[cfg_attr(not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")), allow(dead_code))]
pub(crate) fn parse_kdeglobals(text: &str) -> Option<f32> {
    let mut in_general = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_general = line == "[General]";
            continue;
        }
        if !in_general {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() != "font" {
            continue;
        }
        let points: f64 = value.split(',').nth(1)?.trim().parse().ok()?;
        return valid(points / KDE_DEFAULT_POINTS);
    }
    None
}

/// `gsettings get org.gnome.desktop.interface text-scaling-factor` output (`1.25`) as a factor;
/// 1.0 when it isn't a usable one.
#[cfg_attr(not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")), allow(dead_code))]
pub(crate) fn parse_gsettings(output: &str) -> f32 {
    output.trim().parse::<f64>().ok().and_then(valid).unwrap_or(1.0)
}

/// `reg query HKCU\Software\Microsoft\Accessibility /v TextScaleFactor` output
/// (`TextScaleFactor    REG_DWORD    0x7d`) as a factor; 1.0 when missing or outside 100–225 %.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_reg_query(output: &str) -> f32 {
    output
        .lines()
        .map(str::split_whitespace)
        .find_map(|mut words| {
            (words.next()? == "TextScaleFactor").then_some(())?;
            let hex = words.last()?.strip_prefix("0x")?;
            u32::from_str_radix(hex, 16).ok()
        })
        .filter(|percent| (100..=225).contains(percent))
        .map_or(1.0, |percent| percent as f32 / 100.0)
}

/// Run `program` and return what it printed, or `None` if it is missing, fails or outlives
/// `timeout`.
#[cfg(any(windows, target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
fn run_with_timeout(program: &str, args: &[&str], timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up at launch.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(std::time::Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    // The answer fits in far less; a misbehaving tool can't make launch read without end.
    child.stdout.take()?.take(4096).read_to_string(&mut out).ok()?;
    Some(out)
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
mod unix {
    use std::path::PathBuf;
    use std::time::Duration;

    /// KDE's general font size when KDE is the desktop (or, with no desktop named, when
    /// `kdeglobals` sets one); GNOME's text scaling factor otherwise.
    pub(super) fn system_text_scale() -> f32 {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let kde = kdeglobals().as_deref().and_then(super::parse_kdeglobals);
        if desktop.to_ascii_uppercase().contains("KDE") {
            return kde.unwrap_or(1.0);
        }
        if let (true, Some(scale)) = (desktop.is_empty(), kde) {
            return scale;
        }
        super::run_with_timeout("gsettings", &["get", "org.gnome.desktop.interface", "text-scaling-factor"], Duration::from_millis(500))
            .map_or(1.0, |out| super::parse_gsettings(&out))
    }

    /// The text of `$XDG_CONFIG_HOME/kdeglobals` (default `~/.config/kdeglobals`), if readable.
    fn kdeglobals() -> Option<String> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        let text = std::fs::read(config.join("kdeglobals")).ok()?;
        // Real files are a few KB; don't trust an arbitrary one to be small.
        (text.len() <= 1 << 20).then(|| String::from_utf8_lossy(&text).into_owned())
    }
}

#[cfg(windows)]
mod windows {
    use std::time::Duration;

    /// Settings ▸ Accessibility ▸ Text size, read through `reg query` to avoid a registry crate.
    pub(super) fn system_text_scale() -> f32 {
        super::run_with_timeout(
            "reg",
            &["query", r"HKCU\Software\Microsoft\Accessibility", "/v", "TextScaleFactor"],
            Duration::from_millis(500),
        )
        .map_or(1.0, |out| super::parse_reg_query(&out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_a_usable_system_scale_only() {
        assert_eq!(zoom(None, 1.25, 1.0), 1.25);
        assert_eq!(zoom(None, 1.25, 2.0), 1.25, "Auto keeps the display's scale");
        for system in [f32::NAN, f32::INFINITY, -1.0, 0.0, 0.1, 40.0] {
            assert_eq!(zoom(None, system, 1.0), 1.0, "{system}");
        }
    }

    #[test]
    fn a_chosen_size_is_absolute_whatever_the_display_reports() {
        for native in [1.0, 1.25, 2.0, 4.0] {
            assert_eq!(native * zoom(Some(1.0), 1.25, native), 1.0, "100% on a {native}× display");
            assert_eq!(native * zoom(Some(1.5), 1.0, native), 1.5, "150% on a {native}× display");
        }
        for native in [f32::NAN, f32::INFINITY, 0.0, -2.0] {
            assert_eq!(zoom(Some(1.25), 1.0, native), 1.25, "{native}");
        }
    }

    #[test]
    fn a_saved_size_out_of_range_falls_back_to_auto() {
        for bad in [f32::NAN, 1e30, 0.1, -1.0] {
            assert_eq!(zoom(Some(bad), 1.4, 1.0), 1.4, "{bad}");
        }
    }

    #[test]
    fn presets_are_valid_and_show_as_themselves() {
        for p in PRESETS {
            let factor = valid(f64::from(p) / 100.0).expect("preset in range");
            assert_eq!(percent(factor), u32::from(p));
        }
    }

    #[test]
    fn sync_sets_the_zoom_and_turns_off_keyboard_zoom() {
        let ctx = egui::Context::default();
        let mut applied = None;
        sync(&ctx, 1.25, &mut applied);
        assert_eq!(applied, Some(1.25));
        assert!(!ctx.options(|o| o.zoom_with_keyboard));
    }

    #[test]
    fn kdeglobals_font_size_over_ten_points() {
        let text = "[ColorEffects:Disabled]\nfont=Wrong,20,-1\n\n[General]\nColorScheme=BreezeDark\nfont=Noto Sans,14,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\n\n[KDE]\nfont=Wrong,30\n";
        assert_eq!(parse_kdeglobals(text), Some(1.4));
        assert_eq!(parse_kdeglobals("[General]\nfont = Noto Sans,10,-1,5,50,0,0,0,0,0\n"), Some(1.0));
        assert_eq!(parse_kdeglobals("[General]\r\nfont=Inter,12.5,-1\r\n"), Some(1.25));
    }

    #[test]
    fn kdeglobals_without_a_usable_font_has_no_scale() {
        for bad in [
            "",
            "[General]\nColorScheme=Breeze\n",
            "[KDE]\nfont=Noto Sans,14\n",
            "[General]\nfont=Noto Sans\n",
            "[General]\nfont=Noto Sans,big,-1\n",
            "[General]\nfont=Noto Sans,-1,12\n",
            "[General]\nfont=Noto Sans,400,-1\n",
        ] {
            assert_eq!(parse_kdeglobals(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn gsettings_output_parses_or_means_no_scaling() {
        assert_eq!(parse_gsettings("1.25\n"), 1.25);
        assert_eq!(parse_gsettings("1.0"), 1.0);
        for bad in ["", "nan", "inf", "-2", "0", "uint32 0", "100"] {
            assert_eq!(parse_gsettings(bad), 1.0, "{bad}");
        }
    }

    #[test]
    fn reg_query_output_parses_or_means_no_scaling() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Accessibility\r\n    TextScaleFactor    REG_DWORD    0x7d\r\n\r\n";
        assert_eq!(parse_reg_query(out), 1.25);
        assert_eq!(parse_reg_query("    TextScaleFactor    REG_DWORD    0xe1\n"), 2.25);
        for bad in [
            "",
            "ERROR: The system was unable to find the specified registry key or value.",
            "    TextScaleFactor    REG_DWORD    0x63\n",
            "    TextScaleFactor    REG_DWORD    0xe2\n",
            "    TextScaleFactor    REG_DWORD    zz\n",
            "    OtherValue    REG_DWORD    0x7d\n",
        ] {
            assert_eq!(parse_reg_query(bad), 1.0, "{bad:?}");
        }
    }

    #[test]
    fn reading_the_system_scale_never_fails() {
        let f = system_text_scale();
        assert!(f.is_finite() && (MIN..=MAX).contains(&f), "{f}");
    }
}
