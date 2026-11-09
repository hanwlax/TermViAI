//! Window-local appearance. Preview never rewrites preferences or terminal content.
use serde::{Deserialize, Serialize};
use wezterm_term::color::ColorPalette;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Navy = 1,
    Light = 2,
    #[default]
    #[serde(other)]
    Ink = 0,
}

pub struct Colors {
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

impl Colors {
    pub fn color(value: &str) -> gpui::Hsla {
        gpui::rgb(u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap()).into()
    }
}
