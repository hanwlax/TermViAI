//! Window-local appearance. Preview never rewrites preferences or terminal content.
use super::TermWindow;
use mux::pane::Pane;
use serde::{Deserialize, Serialize};
use wezterm_term::color::ColorPalette;
use wezterm_term::TerminalConfiguration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Theme {
    Navy = 1,
    Light = 2,
    #[default]
    #[serde(other)]
    Ink = 0,
}

pub(super) struct Colors {
    pub bg: &'static str,
    pub top: &'static str,
    pub side: &'static str,
    pub card: &'static str,
    pub field: &'static str,
    pub input_selection: &'static str,
    pub hover: &'static str,
    pub icon_hover: &'static str,
    pub line: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub accent: &'static str,
    pub soft: &'static str,
    pub purple: &'static str,
    pub green: &'static str,
    pub red: &'static str,
    pub blue: &'static str,
    pub terminal: &'static str,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::Ink, Self::Navy, Self::Light];
    pub fn label(self) -> &'static str {
        match self {
            Self::Ink => "墨黑 · 鸢尾紫",
            Self::Navy => "深海 · 冰蓝",
            Self::Light => "瓷白 · 鸢尾紫",
        }
    }
    pub fn colors(self) -> &'static Colors {
        match self {
            Self::Ink => &Colors {
                bg: "#101114",
                top: "#0c0d10",
                side: "#15161a",
                card: "#1b1d22",
                field: "#111216",
                input_selection: "#6e50b4",
                hover: "#25282f",
                icon_hover: "#363a44",
                line: "#30333b",
                text: "#f1f2f5",
                muted: "#a4a9b5",
                accent: "#a78bfa",
                soft: "#282238",
                purple: "#b49af8",
                green: "#74d6ad",
                red: "#ff8996",
                blue: "#83bbf7",
                terminal: "#0c0d10",
            },
            Self::Navy => &Colors {
                bg: "#0a1420",
                top: "#070f18",
                side: "#0e1a28",
                card: "#142335",
                field: "#0a1522",
                input_selection: "#356fa6",
                hover: "#1e3249",
                icon_hover: "#314b66",
                line: "#2b4159",
                text: "#edf5ff",
                muted: "#a3b8cf",
                accent: "#83bcfa",
                soft: "#1b324a",
                purple: "#bba4fa",
                green: "#77d7c1",
                red: "#ff96a3",
                blue: "#90caff",
                terminal: "#08111b",
            },
            Self::Light => &Colors {
                bg: "#f6f7f9",
                top: "#ffffff",
                side: "#eceef2",
                card: "#ffffff",
                field: "#fafbfc",
                input_selection: "#9d83ce",
                hover: "#e8eaf0",
                icon_hover: "#d6dae5",
                line: "#d1d6df",
                text: "#1b2230",
                muted: "#596375",
                accent: "#7252c4",
                soft: "#ede7fa",
                purple: "#7252c4",
                green: "#197453",
                red: "#ba344c",
                blue: "#2465a6",
                terminal: "#ffffff",
            },
        }
    }

    /// Existing chrome colors are semantic keys; translate all surfaces at paint time.
    pub fn resolve(self, color: &str) -> &str {
        let c = self.colors();
        match color {
            "#1c1d2b" => c.bg,
            "#171823" => c.top,
            "#232435" => c.side,
            "#292a3c" | "#2c2d41" => c.card,
            "#37394f" => c.field,
            "#52546e" | "#414256" => c.hover,
            "#5a5c76" => c.icon_hover,
            "#353648" | "#36374a" | "#38394c" | "#3b3e52" | "#45445c" => c.line,
            "#f1f2fa" | "#f1efff" => c.text,
            "#9ca2ba" | "#b4b8cd" | "#b5b8cc" | "#b8bdd2" | "#53566c" => c.muted,
            "#aaa7cb" | "#8d86d7" | "#a6a0ff" => c.accent,
            "#474159" | "#575078" | "#34314f" | "#302d47" | "#514b73" | "#4a436c" => c.soft,
            "#b5a2d8" => c.accent,
            "#b8b2ff" | "#7568df" => c.purple,
            "#89b4fa" => c.blue,
            "#59b79e" => c.green,
            "#285d53" => {
                if self == Self::Light {
                    "#dff3e8"
                } else {
                    "#163b30"
                }
            }
            "#ed8193" | "#b84e64" | "#b65370" | "#f5a6b8" => c.red,
            "#59333f" | "#3b2b3c" => {
                if self == Self::Light {
                    "#fce7ec"
                } else {
                    "#47232c"
                }
            }
            "#ad4f60" => {
                if self == Self::Light {
                    "#ba344c"
                } else {
                    "#a43b50"
                }
            }
            "#fff2f3" => "#ffffff",
            "#080911" | "#090a12" | "#12131e" => "#080911",
            "#343648" | "#414357" | "#45485e" => c.hover,
            _ => color,
        }
    }

    pub fn terminal_palette(self) -> ColorPalette {
        static PALETTES: std::sync::OnceLock<[ColorPalette; 3]> = std::sync::OnceLock::new();
        PALETTES.get_or_init(|| {
            Self::ALL.map(|theme| {
                let c = theme.colors();
                let rgb = |s: &str| s.parse::<wezterm_term::color::SrgbaTuple>().unwrap();
                let mut p = ColorPalette::default();
                p.background = rgb(c.terminal);
                p.foreground = rgb(c.text);
                p.cursor_bg = rgb(c.text);
                p.cursor_fg = rgb(c.terminal);
                p.cursor_border = rgb(c.accent);
                p.selection_bg = rgb(if theme == Self::Light {
                    "#ddd2f4"
                } else {
                    "#423858"
                });
                p.selection_fg = rgb(c.text);
                p.scrollbar_thumb = rgb(c.muted);
                p.split = rgb(c.line);
                let ansi = if theme == Self::Light {
                    [
                        "#303744", c.red, c.green, "#886000", c.blue, c.purple, "#197078",
                        "#596375", "#687385", "#a62039", "#146044", "#755100", "#1d5390",
                        "#6040ad", "#145d65", "#1b2230",
                    ]
                } else {
                    [
                        "#242830", c.red, c.green, "#e6c27a", c.blue, c.purple, "#79c8ce", c.text,
                        "#8c96a8", "#ffabb4", "#9ce6c7", "#f0d99a", "#add4ff", "#d0bafa",
                        "#a0e0e5", "#ffffff",
                    ]
                };
                for (index, value) in ansi.iter().enumerate() {
                    p.colors.0[index] = rgb(value);
                }
                p
            })
        })[self as usize]
            .clone()
    }
}

/// Preserve explicit OSC palette changes and extended 256-color entries.
fn themed_palette(
    mut pane: ColorPalette,
    base: &ColorPalette,
    themed: &ColorPalette,
) -> ColorPalette {
    macro_rules! inherit { ($($field:ident),*) => {$(if pane.$field == base.$field { pane.$field = themed.$field; })*}; }
    inherit!(
        foreground,
        background,
        cursor_fg,
        cursor_bg,
        cursor_border,
        selection_fg,
        selection_bg,
        scrollbar_thumb,
        split
    );
    for i in 0..16 {
        if pane.colors.0[i] == base.colors.0[i] {
            pane.colors.0[i] = themed.colors.0[i];
        }
    }
    pane
}

impl TermWindow {
    pub(super) fn termviai_theme(&self) -> Theme {
        self.termviai_ui
            .preview_theme()
            .unwrap_or(self.termviai_fonts.theme())
    }
    pub(super) fn termviai_pane_palette(&self, pane: &dyn Pane) -> ColorPalette {
        let original = pane.palette();
        if !self.config.termviai_ui {
            return original;
        }
        let base = config::TermConfig::with_config(self.config.clone()).color_palette();
        themed_palette(original, &base, &self.termviai_theme().terminal_palette())
    }
    pub(super) fn termviai_sync_theme(&mut self) {
        if !self.config.termviai_ui {
            return;
        }
        let theme = self.termviai_theme();
        if self.termviai_fonts.painted_theme != Some(theme) {
            self.termviai_fonts.painted_theme = Some(theme);
            self.palette.take();
            self.quad_generation += 1;
            self.shape_generation += 1;
            self.line_quad_cache.borrow_mut().clear();
            self.line_to_ele_shape_cache.borrow_mut().clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_theme_keeps_text_and_controls_legible() {
        fn luminance(hex: &str) -> f64 {
            let rgb = u32::from_str_radix(&hex[1..], 16).unwrap();
            let linear = |shift| {
                let v = ((rgb >> shift) & 255_u32) as f64 / 255.;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(16) + 0.7152 * linear(8) + 0.0722 * linear(0)
        }
        fn contrast(a: &str, b: &str) -> f64 {
            let a = luminance(a);
            let b = luminance(b);
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for theme in Theme::ALL {
            let c = theme.colors();
            for bg in [c.bg, c.top, c.side, c.card, c.field, c.terminal] {
                for fg in [c.text, c.muted] {
                    assert!(contrast(fg, bg) >= 4.5, "{:?} {} on {}", theme, fg, bg);
                }
                assert!(contrast(c.accent, bg) >= 3., "{:?} accent on {}", theme, bg);
            }
            assert_ne!(c.hover, c.icon_hover);
            assert!(contrast(c.text, c.input_selection) >= 4.5);
            assert!(contrast(c.field, c.input_selection) >= 3.);
        }
    }

    #[test]
    fn palettes_switch_defaults_without_replacing_osc_or_extended_colors() {
        let base = ColorPalette::default();
        for theme in Theme::ALL {
            let target = theme.terminal_palette();
            let mut custom = base.clone();
            custom.colors.0[1] = target.colors.0[6];
            let actual = themed_palette(custom.clone(), &base, &target);
            assert_eq!(actual.background, target.background);
            assert_eq!(actual.foreground, target.foreground);
            assert_eq!(actual.colors.0[1], custom.colors.0[1]);
            assert_eq!(actual.colors.0[18], base.colors.0[18]);
            assert_ne!(actual.foreground, actual.background);
        }
    }
}
