//! TermViAI's font-independent, monochrome interface icons.
//!
//! All paths use a 24-unit square with an inset for the stroke. Pass the physical
//! pixel size to `Icon::content`; the normal element text color tints the result.

use super::box_model::{ElementContent, SizedPoly};
use crate::customglyph::{BlockAlpha, BlockCoord, Poly, PolyCommand, PolyStyle};
use config::Dimension;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Menu,
    Hosts,
    Key,
    Terminal,
    Search,
    Plus,
    Close,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Minimize,
    Maximize,
    Restore,
    Grid,
    List,
    Folder,
    Edit,
    Check,
    Checkbox,
    CheckboxChecked,
    Broadcast,
    SplitRight,
    SplitDown,
    ArrowLeft,
    ArrowRight,
    Settings,
    More,
    Link,
    Power,
}

impl Icon {
    /// An icon has no intrinsic padding or background, so its placement does
    /// not depend on the selected UI font, fallback fonts, or a text baseline.
    pub fn content(self, size_px: f32) -> ElementContent {
        let size_px = size_px.max(1.);
        ElementContent::Poly {
            line_width: (size_px / 12.).round().max(1.) as isize,
            poly: SizedPoly {
                poly: self.poly(),
                width: Dimension::Pixels(size_px),
                height: Dimension::Pixels(size_px),
            },
        }
    }

    pub fn poly(self) -> &'static [Poly] {
        match self {
            Self::Menu => MENU,
            Self::Hosts => HOSTS,
            Self::Key => KEY,
            Self::Terminal => TERMINAL,
            Self::Search => SEARCH,
            Self::Plus => PLUS,
            Self::Close => CLOSE,
            Self::ChevronLeft => CHEVRON_LEFT,
            Self::ChevronRight => CHEVRON_RIGHT,
            Self::ChevronDown => CHEVRON_DOWN,
            Self::ChevronUp => CHEVRON_UP,
            Self::Minimize => MINIMIZE,
            Self::Maximize => MAXIMIZE,
            Self::Restore => RESTORE,
            Self::Grid => GRID,
            Self::List => LIST,
            Self::Folder => FOLDER,
            Self::Edit => EDIT,
            Self::Check => CHECK,
            Self::Checkbox => CHECKBOX,
            Self::CheckboxChecked => CHECKBOX_CHECKED,
            Self::Broadcast => BROADCAST,
            Self::SplitRight => SPLIT_RIGHT,
            Self::SplitDown => SPLIT_DOWN,
            Self::ArrowLeft => ARROW_LEFT,
            Self::ArrowRight => ARROW_RIGHT,
            Self::Settings => SETTINGS,
            Self::More => MORE,
            Self::Link => LINK,
            Self::Power => POWER,
        }
    }
}

const fn coord(value: i8) -> BlockCoord {
    BlockCoord::Frac(value, 24)
}

const fn point(x: i8, y: i8) -> (BlockCoord, BlockCoord) {
    (coord(x), coord(y))
}

const fn m(x: i8, y: i8) -> PolyCommand {
    PolyCommand::MoveTo(coord(x), coord(y))
}

const fn l(x: i8, y: i8) -> PolyCommand {
    PolyCommand::LineTo(coord(x), coord(y))
}

const fn q(cx: i8, cy: i8, x: i8, y: i8) -> PolyCommand {
    PolyCommand::QuadTo {
        control: point(cx, cy),
        to: point(x, y),
    }
}

const fn circle(x: i8, y: i8, radius: i8) -> PolyCommand {
    PolyCommand::Circle {
        center: point(x, y),
        radius: coord(radius),
    }
}

const fn outline(path: &'static [PolyCommand]) -> Poly {
    Poly {
        path,
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    }
}

const fn fill(path: &'static [PolyCommand]) -> Poly {
    Poly {
        path,
        intensity: BlockAlpha::Full,
        style: PolyStyle::Fill,
    }
}

const FRAME: Poly = outline(&[
    m(6, 4),
    l(18, 4),
    q(20, 4, 20, 6),
    l(20, 18),
    q(20, 20, 18, 20),
    l(6, 20),
    q(4, 20, 4, 18),
    l(4, 6),
    q(4, 4, 6, 4),
    PolyCommand::Close,
]);
const TICK: Poly = outline(&[m(7, 12), l(10, 15), l(17, 8)]);

const MENU: &[Poly] = &[outline(&[
    m(4, 6),
    l(20, 6),
    m(4, 12),
    l(20, 12),
    m(4, 18),
    l(20, 18),
])];

const HOSTS: &[Poly] = &[
    outline(&[
        m(5, 3),
        l(19, 3),
        q(21, 3, 21, 5),
        l(21, 9),
        q(21, 11, 19, 11),
        l(5, 11),
        q(3, 11, 3, 9),
        l(3, 5),
        q(3, 3, 5, 3),
        PolyCommand::Close,
        m(5, 13),
        l(19, 13),
        q(21, 13, 21, 15),
        l(21, 19),
        q(21, 21, 19, 21),
        l(5, 21),
        q(3, 21, 3, 19),
        l(3, 15),
        q(3, 13, 5, 13),
        PolyCommand::Close,
        m(11, 7),
        l(17, 7),
        m(11, 17),
        l(17, 17),
    ]),
    fill(&[circle(7, 7, 1), circle(7, 17, 1)]),
];

const KEY: &[Poly] = &[outline(&[
    circle(8, 8, 5),
    m(12, 12),
    l(21, 21),
    m(16, 16),
    l(19, 13),
    m(19, 19),
    l(22, 16),
])];

const TERMINAL: &[Poly] = &[outline(&[
    m(5, 4),
    l(19, 4),
    q(21, 4, 21, 6),
    l(21, 18),
    q(21, 20, 19, 20),
    l(5, 20),
    q(3, 20, 3, 18),
    l(3, 6),
    q(3, 4, 5, 4),
    PolyCommand::Close,
    m(7, 8),
    l(11, 12),
    l(7, 16),
    m(13, 16),
    l(17, 16),
])];

const SEARCH: &[Poly] = &[outline(&[circle(10, 10, 6), m(15, 15), l(21, 21)])];
const PLUS: &[Poly] = &[outline(&[m(12, 5), l(12, 19), m(5, 12), l(19, 12)])];
const CLOSE: &[Poly] = &[outline(&[m(6, 6), l(18, 18), m(18, 6), l(6, 18)])];
const CHEVRON_LEFT: &[Poly] = &[outline(&[m(15, 5), l(8, 12), l(15, 19)])];
const CHEVRON_RIGHT: &[Poly] = &[outline(&[m(9, 5), l(16, 12), l(9, 19)])];
const CHEVRON_DOWN: &[Poly] = &[outline(&[m(5, 9), l(12, 16), l(19, 9)])];
const CHEVRON_UP: &[Poly] = &[outline(&[m(5, 15), l(12, 8), l(19, 15)])];
const POWER: &[Poly] = &[outline(&[
    m(12, 3),
    l(12, 11),
    m(6, 5),
    q(3, 8, 3, 12),
    q(3, 21, 12, 21),
    q(21, 21, 21, 12),
    q(21, 8, 18, 5),
])];
const MINIMIZE: &[Poly] = &[outline(&[m(5, 12), l(19, 12)])];
const MAXIMIZE: &[Poly] = &[FRAME];
const RESTORE: &[Poly] = &[outline(&[
    m(8, 7),
    l(8, 4),
    l(20, 4),
    l(20, 16),
    l(17, 16),
    m(4, 8),
    l(16, 8),
    l(16, 20),
    l(4, 20),
    PolyCommand::Close,
])];

const GRID: &[Poly] = &[outline(&[
    m(4, 4),
    l(10, 4),
    l(10, 10),
    l(4, 10),
    PolyCommand::Close,
    m(14, 4),
    l(20, 4),
    l(20, 10),
    l(14, 10),
    PolyCommand::Close,
    m(4, 14),
    l(10, 14),
    l(10, 20),
    l(4, 20),
    PolyCommand::Close,
    m(14, 14),
    l(20, 14),
    l(20, 20),
    l(14, 20),
    PolyCommand::Close,
])];
const LIST: &[Poly] = &[
    outline(&[m(9, 6), l(21, 6), m(9, 12), l(21, 12), m(9, 18), l(21, 18)]),
    fill(&[circle(4, 6, 1), circle(4, 12, 1), circle(4, 18, 1)]),
];
const FOLDER: &[Poly] = &[outline(&[
    m(3, 7),
    q(3, 5, 5, 5),
    l(10, 5),
    l(12, 8),
    l(19, 8),
    q(21, 8, 21, 10),
    l(21, 18),
    q(21, 20, 19, 20),
    l(5, 20),
    q(3, 20, 3, 18),
    PolyCommand::Close,
])];
const EDIT: &[Poly] = &[outline(&[
    m(4, 15),
    l(15, 4),
    q(17, 2, 19, 4),
    l(20, 5),
    q(22, 7, 20, 9),
    l(9, 20),
    l(3, 21),
    PolyCommand::Close,
    m(13, 6),
    l(18, 11),
    m(4, 15),
    l(9, 20),
])];
const CHECK: &[Poly] = &[outline(&[m(4, 12), l(9, 17), l(20, 6)])];
const CHECKBOX: &[Poly] = &[FRAME];
const CHECKBOX_CHECKED: &[Poly] = &[FRAME, TICK];

const BROADCAST: &[Poly] = &[
    outline(&[
        m(5, 4),
        q(1, 8, 2, 13),
        q(2, 16, 5, 19),
        m(19, 4),
        q(23, 8, 22, 13),
        q(22, 16, 19, 19),
        m(8, 7),
        q(4, 11, 8, 15),
        m(16, 7),
        q(20, 11, 16, 15),
        m(12, 14),
        l(12, 21),
        m(8, 21),
        l(16, 21),
    ]),
    fill(&[circle(12, 11, 2)]),
];

const SPLIT_RIGHT: &[Poly] = &[FRAME, outline(&[m(12, 4), l(12, 20)])];
const SPLIT_DOWN: &[Poly] = &[FRAME, outline(&[m(4, 12), l(20, 12)])];
const ARROW_LEFT: &[Poly] = &[outline(&[
    m(11, 5),
    l(4, 12),
    l(11, 19),
    m(4, 12),
    l(21, 12),
])];
const ARROW_RIGHT: &[Poly] = &[outline(&[
    m(13, 5),
    l(20, 12),
    l(13, 19),
    m(20, 12),
    l(3, 12),
])];

// Sliders stay readable at small sizes while communicating preferences.
const SETTINGS: &[Poly] = &[outline(&[
    m(3, 6),
    l(8, 6),
    m(12, 6),
    l(21, 6),
    circle(10, 6, 2),
    m(3, 12),
    l(13, 12),
    m(17, 12),
    l(21, 12),
    circle(15, 12, 2),
    m(3, 18),
    l(6, 18),
    m(10, 18),
    l(21, 18),
    circle(8, 18, 2),
])];
const MORE: &[Poly] = &[fill(&[
    circle(5, 12, 1),
    circle(12, 12, 1),
    circle(19, 12, 1),
])];
const LINK: &[Poly] = &[outline(&[
    m(10, 8),
    l(13, 5),
    q(17, 1, 21, 5),
    q(24, 9, 20, 12),
    l(17, 15),
    m(14, 16),
    l(11, 19),
    q(7, 23, 3, 19),
    q(0, 15, 4, 12),
    l(7, 9),
    m(8, 16),
    l(16, 8),
])];

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Icon] = &[
        Icon::Menu,
        Icon::Hosts,
        Icon::Key,
        Icon::Terminal,
        Icon::Search,
        Icon::Plus,
        Icon::Close,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::ChevronUp,
        Icon::Minimize,
        Icon::Maximize,
        Icon::Restore,
        Icon::Grid,
        Icon::List,
        Icon::Folder,
        Icon::Edit,
        Icon::Check,
        Icon::Checkbox,
        Icon::CheckboxChecked,
        Icon::Broadcast,
        Icon::SplitRight,
        Icon::SplitDown,
        Icon::ArrowLeft,
        Icon::ArrowRight,
        Icon::Settings,
        Icon::More,
        Icon::Link,
        Icon::Power,
    ];

    fn value(coord: BlockCoord) -> f32 {
        match coord {
            BlockCoord::Frac(n, 24) => n as f32,
            _ => panic!("Icons must use the same normalized 24-unit grid"),
        }
    }

    fn check_point(point: (BlockCoord, BlockCoord)) {
        assert!((0. ..=24.).contains(&value(point.0)));
        assert!((0. ..=24.).contains(&value(point.1)));
    }

    #[test]
    fn vector_icons_have_valid_paths_at_multiple_dpi_scales() {
        for icon in ALL {
            assert!(!icon.poly().is_empty(), "{:?}", icon);
            for poly in icon.poly() {
                assert!(!poly.path.is_empty());
                let mut has_start = false;
                for command in poly.path {
                    match *command {
                        PolyCommand::MoveTo(x, y) => {
                            check_point((x, y));
                            has_start = true;
                        }
                        PolyCommand::LineTo(x, y) => {
                            assert!(has_start, "{:?} draws before MoveTo", icon);
                            check_point((x, y));
                        }
                        PolyCommand::QuadTo { control, to } => {
                            assert!(has_start);
                            check_point(control);
                            check_point(to);
                        }
                        PolyCommand::Circle { center, radius } => {
                            check_point(center);
                            let radius = value(radius);
                            assert!(radius > 0.);
                            for coord in [center.0, center.1] {
                                assert!(value(coord) - radius >= 1.);
                                assert!(value(coord) + radius <= 23.);
                            }
                        }
                        PolyCommand::Close => assert!(has_start),
                        PolyCommand::Oval { .. } => panic!("Use Circle for square icons"),
                    }
                }
            }
            for size_px in [16., 20., 24., 30., 40., 48.] {
                match icon.content(size_px) {
                    ElementContent::Poly { line_width, poly } => {
                        assert!(line_width >= 1);
                        assert_eq!(poly.width, Dimension::Pixels(size_px));
                        assert_eq!(poly.height, Dimension::Pixels(size_px));
                    }
                    _ => panic!("Icons must not rely on font glyphs"),
                }
            }
        }
    }
}
