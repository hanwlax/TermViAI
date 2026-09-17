use crate::domain::DomainId;
use crate::pane::*;
use crate::renderable::StableCursorPosition;
use crate::{Mux, MuxNotification, WindowId};
use bintree::PathBranch;
use config::configuration;
use config::keyassignment::PaneDirection;
use parking_lot::Mutex;
use rangeset::intersects_range;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use url::Url;
use wezterm_term::{StableRowIndex, TerminalSize};

pub type Tree = bintree::Tree<Arc<dyn Pane>, SplitDirectionAndSize>;
pub type Cursor = bintree::Cursor<Arc<dyn Pane>, SplitDirectionAndSize>;

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type TabId = usize;

#[derive(Clone, Default)]
struct Recency {
    count: usize,
    by_idx: HashMap<usize, usize>,
}

impl Recency {
    fn tag(&mut self, idx: usize) {
        self.by_idx.insert(idx, self.count);
        self.count += 1;
    }

    fn score(&self, idx: usize) -> usize {
        self.by_idx.get(&idx).copied().unwrap_or(0)
    }
}

struct TabInner {
    id: TabId,
    pane: Option<Tree>,
    size: TerminalSize,
    size_before_zoom: TerminalSize,
    // GUI chrome occupies outer layout rows, not PTY rows. Zero preserves the
    // traditional mux layout for clients that do not opt in.
    pane_header_rows: usize,
    // Symmetric physical-pixel inset around each pane's terminal content.
    // This is opt-in native GUI chrome; remote mux layouts leave it at zero.
    pane_content_padding: (usize, usize),
    // Opt-in GUI sizing; remote mux layouts retain their existing cell semantics.
    proportional_resize: bool,
    resize_reference: Option<SplitResizeReference>,
    pane_cell_sizes: HashMap<PaneId, (usize, usize)>,
    active: usize,
    zoomed: Option<Arc<dyn Pane>>,
    title: String,
    recency: Recency,
}

/// A Tab is a container of Panes
pub struct Tab {
    inner: Mutex<TabInner>,
    tab_id: TabId,
}

fn clone_pane_tree(tree: &Tree) -> Tree {
    match tree {
        Tree::Empty => Tree::Empty,
        Tree::Leaf(pane) => Tree::Leaf(Arc::clone(pane)),
        Tree::Node { left, right, data } => Tree::Node {
            left: Box::new(clone_pane_tree(left)),
            right: Box::new(clone_pane_tree(right)),
            data: *data,
        },
    }
}

#[derive(Clone)]
pub struct PositionedPane {
    /// The topological pane index that can be used to reference this pane
    pub index: usize,
    /// true if this is the active pane at the time the position was computed
    pub is_active: bool,
    /// true if this pane is zoomed
    pub is_zoomed: bool,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub left: usize,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub top: usize,
    /// The width of this pane in cells
    pub width: usize,
    pub pixel_width: usize,
    /// The height of this pane in cells
    pub height: usize,
    pub pixel_height: usize,
    /// The pane instance
    pub pane: Arc<dyn Pane>,
}

impl std::fmt::Debug for PositionedPane {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("PositionedPane")
            .field("index", &self.index)
            .field("is_active", &self.is_active)
            .field("left", &self.left)
            .field("top", &self.top)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pane_id", &self.pane.pane_id())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// The size is of the (first, second) child of the split
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct SplitDirectionAndSize {
    pub direction: SplitDirection,
    pub first: TerminalSize,
    pub second: TerminalSize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum SplitSize {
    Cells(usize),
    Percent(u8),
}

impl Default for SplitSize {
    fn default() -> Self {
        Self::Percent(50)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct SplitRequest {
    pub direction: SplitDirection,
    /// Whether the newly created item will be in the second part
    /// of the split (right/bottom)
    pub target_is_second: bool,
    /// Split across the top of the tab rather than the active pane
    pub top_level: bool,
    /// The size of the new item
    pub size: SplitSize,
}

impl Default for SplitRequest {
    fn default() -> Self {
        Self {
            direction: SplitDirection::Horizontal,
            target_is_second: true,
            top_level: false,
            size: SplitSize::default(),
        }
    }
}

impl SplitDirectionAndSize {
    fn top_of_second(&self) -> usize {
        match self.direction {
            SplitDirection::Horizontal => 0,
            SplitDirection::Vertical => self.first.rows as usize + 1,
        }
    }

    fn left_of_second(&self) -> usize {
        match self.direction {
            SplitDirection::Horizontal => self.first.cols as usize + 1,
            SplitDirection::Vertical => 0,
        }
    }

    pub fn width(&self) -> usize {
        if self.direction == SplitDirection::Horizontal {
            self.first.cols + self.second.cols + 1
        } else {
            self.first.cols
        }
    }

    pub fn height(&self) -> usize {
        if self.direction == SplitDirection::Vertical {
            self.first.rows + self.second.rows + 1
        } else {
            self.first.rows
        }
    }

    pub fn size(&self) -> TerminalSize {
        let cell_width = self.first.pixel_width / self.first.cols;
        let cell_height = self.first.pixel_height / self.first.rows;

        let rows = self.height();
        let cols = self.width();

        TerminalSize {
            rows,
            cols,
            pixel_height: cell_height * rows,
            pixel_width: cell_width * cols,
            dpi: self.first.dpi,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PositionedSplit {
    /// The topological node index that can be used to reference this split
    pub index: usize,
    pub direction: SplitDirection,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this split, in cells.
    pub left: usize,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this split, in cells.
    pub top: usize,
    /// For Horizontal splits, how tall the split should be, for Vertical
    /// splits how wide it should be
    pub size: usize,
}

fn is_pane(pane: &Arc<dyn Pane>, other: &Option<&Arc<dyn Pane>>) -> bool {
    if let Some(other) = other {
        other.pane_id() == pane.pane_id()
    } else {
        false
    }
}

fn pane_tree(
    tree: &Tree,
    tab_id: TabId,
    window_id: WindowId,
    active: Option<&Arc<dyn Pane>>,
    zoomed: Option<&Arc<dyn Pane>>,
    workspace: &str,
    left_col: usize,
    top_row: usize,
) -> PaneNode {
    match tree {
        Tree::Empty => PaneNode::Empty,
        Tree::Node { left, right, data } => {
            let data = data.unwrap();
            PaneNode::Split {
                left: Box::new(pane_tree(
                    &*left, tab_id, window_id, active, zoomed, workspace, left_col, top_row,
                )),
                right: Box::new(pane_tree(
                    &*right,
                    tab_id,
                    window_id,
                    active,
                    zoomed,
                    workspace,
                    if data.direction == SplitDirection::Vertical {
                        left_col
                    } else {
                        left_col + data.left_of_second()
                    },
                    if data.direction == SplitDirection::Horizontal {
                        top_row
                    } else {
                        top_row + data.top_of_second()
                    },
                )),
                node: data,
            }
        }
        Tree::Leaf(pane) => {
            let dims = pane.get_dimensions();
            let working_dir = pane.get_current_working_dir(CachePolicy::AllowStale);
            let cursor_pos = pane.get_cursor_position();

            PaneNode::Leaf(PaneEntry {
                window_id,
                tab_id,
                pane_id: pane.pane_id(),
                title: pane.get_title(),
                is_active_pane: is_pane(pane, &active),
                is_zoomed_pane: is_pane(pane, &zoomed),
                size: TerminalSize {
                    cols: dims.cols,
                    rows: dims.viewport_rows,
                    pixel_height: dims.pixel_height,
                    pixel_width: dims.pixel_width,
                    dpi: dims.dpi,
                },
                working_dir: working_dir.map(Into::into),
                workspace: workspace.to_string(),
                cursor_pos,
                physical_top: dims.physical_top,
                left_col,
                top_row,
                tty_name: pane.tty_name(),
            })
        }
    }
}

fn build_from_pane_tree<F>(
    tree: bintree::Tree<PaneEntry, SplitDirectionAndSize>,
    active: &mut Option<Arc<dyn Pane>>,
    zoomed: &mut Option<Arc<dyn Pane>>,
    make_pane: &mut F,
) -> Tree
where
    F: FnMut(PaneEntry) -> Arc<dyn Pane>,
{
    match tree {
        bintree::Tree::Empty => Tree::Empty,
        bintree::Tree::Node { left, right, data } => Tree::Node {
            left: Box::new(build_from_pane_tree(*left, active, zoomed, make_pane)),
            right: Box::new(build_from_pane_tree(*right, active, zoomed, make_pane)),
            data,
        },
        bintree::Tree::Leaf(entry) => {
            let is_zoomed_pane = entry.is_zoomed_pane;
            let is_active_pane = entry.is_active_pane;
            let pane = make_pane(entry);
            if is_zoomed_pane {
                zoomed.replace(Arc::clone(&pane));
            }
            if is_active_pane {
                active.replace(Arc::clone(&pane));
            }
            Tree::Leaf(pane)
        }
    }
}

/// Computes the minimum (x, y) size based on the panes in this portion
/// of the tree.
fn compute_min_size(tree: &Tree) -> (usize, usize) {
    match tree {
        Tree::Node { data: None, .. } | Tree::Empty => (1, 1),
        Tree::Node {
            left,
            right,
            data: Some(data),
        } => {
            let (left_x, left_y) = compute_min_size(left);
            let (right_x, right_y) = compute_min_size(right);
            match data.direction {
                SplitDirection::Vertical => (left_x.max(right_x), left_y + right_y + 1),
                SplitDirection::Horizontal => (left_x + right_x + 1, left_y.max(right_y)),
            }
        }
        Tree::Leaf(_) => (1, 1),
    }
}

/// The ratios describe a user-selected layout, rather than the last rounded cell
/// allocation. Keeping them across window resize events prevents one-cell steps
/// and temporarily clamped minimum sizes from gradually moving the dividers.
#[derive(Clone)]
enum SplitResizeReference {
    Empty,
    Pane(PaneId),
    Split {
        direction: SplitDirection,
        first: usize,
        second: usize,
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl SplitResizeReference {
    fn capture(tree: &Tree) -> Self {
        match tree {
            Tree::Empty | Tree::Node { data: None, .. } => Self::Empty,
            Tree::Leaf(pane) => Self::Pane(pane.pane_id()),
            Tree::Node {
                left,
                right,
                data: Some(data),
            } => {
                let (first, second) = split_dimensions(data);
                Self::Split {
                    direction: data.direction,
                    first,
                    second,
                    left: Box::new(Self::capture(left)),
                    right: Box::new(Self::capture(right)),
                }
            }
        }
    }

    fn matches(&self, tree: &Tree) -> bool {
        match (self, tree) {
            (Self::Empty, Tree::Empty | Tree::Node { data: None, .. }) => true,
            (Self::Pane(id), Tree::Leaf(pane)) => *id == pane.pane_id(),
            (
                Self::Split {
                    direction,
                    left,
                    right,
                    ..
                },
                Tree::Node {
                    left: tree_left,
                    right: tree_right,
                    data: Some(data),
                },
            ) => {
                *direction == data.direction && left.matches(tree_left) && right.matches(tree_right)
            }
            _ => false,
        }
    }

    /// A divider drag changes this node's ratio, not its descendants' ratios.
    fn update_split(&mut self, tree: &Tree) -> bool {
        if self.matches(tree) {
            if let (
                Self::Split { first, second, .. },
                Tree::Node {
                    data: Some(data), ..
                },
            ) = (self, tree)
            {
                (*first, *second) = split_dimensions(data);
                return true;
            }
            return false;
        }
        if let Self::Split { left, right, .. } = self {
            return left.update_split(tree) || right.update_split(tree);
        }
        false
    }

    fn apply(&self, tree: &mut Tree, size: TerminalSize) {
        if let (
            Self::Split {
                first,
                second,
                left: reference_left,
                right: reference_right,
                ..
            },
            Tree::Node {
                left,
                right,
                data: Some(data),
            },
        ) = (self, tree)
        {
            let minimum_first = compute_min_size(left);
            let minimum_second = compute_min_size(right);
            resize_split_proportionally(data, size, *first, *second, minimum_first, minimum_second);
            reference_left.apply(left, data.first);
            reference_right.apply(right, data.second);
        }
    }
}

fn split_dimensions(data: &SplitDirectionAndSize) -> (usize, usize) {
    match data.direction {
        SplitDirection::Horizontal => (data.first.cols, data.second.cols),
        SplitDirection::Vertical => (data.first.rows, data.second.rows),
    }
}

fn proportional_first_size(
    span: usize,
    first: usize,
    second: usize,
    minimum_first: usize,
    minimum_second: usize,
) -> usize {
    // Use integers and round only at allocation time. u128 avoids intermediate
    // multiplication overflow on either 32-bit or 64-bit targets.
    let total = first as u128 + second as u128;
    let first = if total == 0 {
        span / 2
    } else {
        ((span as u128 * first as u128 + total / 2) / total) as usize
    };
    first
        .max(minimum_first)
        .min(span.saturating_sub(minimum_second))
}

fn resize_split_proportionally(
    data: &mut SplitDirectionAndSize,
    size: TerminalSize,
    first: usize,
    second: usize,
    minimum_first: (usize, usize),
    minimum_second: (usize, usize),
) {
    let dims = cell_dimensions(&size);
    data.first = size;
    data.second = size;
    match data.direction {
        SplitDirection::Horizontal => {
            let available = size.cols.saturating_sub(1);
            data.first.cols = proportional_first_size(
                available,
                first,
                second,
                minimum_first.0,
                minimum_second.0,
            );
            data.second.cols = available - data.first.cols;
        }
        SplitDirection::Vertical => {
            let available = size.rows.saturating_sub(1);
            data.first.rows = proportional_first_size(
                available,
                first,
                second,
                minimum_first.1,
                minimum_second.1,
            );
            data.second.rows = available - data.first.rows;
        }
    }
    data.first.pixel_width = data.first.cols * dims.pixel_width;
    data.first.pixel_height = data.first.rows * dims.pixel_height;
    data.second.pixel_width = data.second.cols * dims.pixel_width;
    data.second.pixel_height = data.second.rows * dims.pixel_height;
}

fn adjust_x_size(tree: &mut Tree, mut x_adjust: isize, cell_dimensions: &TerminalSize) {
    let (min_x, _) = compute_min_size(tree);
    while x_adjust != 0 {
        match tree {
            Tree::Empty | Tree::Leaf(_) => return,
            Tree::Node { data: None, .. } => return,
            Tree::Node {
                left,
                right,
                data: Some(data),
            } => {
                data.first.dpi = cell_dimensions.dpi;
                data.second.dpi = cell_dimensions.dpi;
                match data.direction {
                    SplitDirection::Vertical => {
                        let new_cols = (data.first.cols as isize)
                            .saturating_add(x_adjust)
                            .max(min_x as isize);
                        x_adjust = new_cols.saturating_sub(data.first.cols as isize);

                        if x_adjust != 0 {
                            adjust_x_size(&mut *left, x_adjust, cell_dimensions);
                            data.first.cols = new_cols.try_into().unwrap();
                            data.first.pixel_width =
                                data.first.cols.saturating_mul(cell_dimensions.pixel_width);

                            adjust_x_size(&mut *right, x_adjust, cell_dimensions);
                            data.second.cols = data.first.cols;
                            data.second.pixel_width = data.first.pixel_width;
                        }
                        return;
                    }
                    SplitDirection::Horizontal if x_adjust > 0 => {
                        adjust_x_size(&mut *left, 1, cell_dimensions);
                        data.first.cols += 1;
                        data.first.pixel_width =
                            data.first.cols.saturating_mul(cell_dimensions.pixel_width);
                        x_adjust -= 1;

                        if x_adjust > 0 {
                            adjust_x_size(&mut *right, 1, cell_dimensions);
                            data.second.cols += 1;
                            data.second.pixel_width =
                                data.second.cols.saturating_mul(cell_dimensions.pixel_width);
                            x_adjust -= 1;
                        }
                    }
                    SplitDirection::Horizontal => {
                        // x_adjust is negative
                        if data.first.cols > 1 {
                            adjust_x_size(&mut *left, -1, cell_dimensions);
                            data.first.cols -= 1;
                            data.first.pixel_width =
                                data.first.cols.saturating_mul(cell_dimensions.pixel_width);
                            x_adjust += 1;
                        }
                        if x_adjust < 0 && data.second.cols > 1 {
                            adjust_x_size(&mut *right, -1, cell_dimensions);
                            data.second.cols -= 1;
                            data.second.pixel_width =
                                data.second.cols.saturating_mul(cell_dimensions.pixel_width);
                            x_adjust += 1;
                        }
                    }
                }
            }
        }
    }
}

fn adjust_y_size(tree: &mut Tree, mut y_adjust: isize, cell_dimensions: &TerminalSize) {
    let (_, min_y) = compute_min_size(tree);
    while y_adjust != 0 {
        match tree {
            Tree::Empty | Tree::Leaf(_) => return,
            Tree::Node { data: None, .. } => return,
            Tree::Node {
                left,
                right,
                data: Some(data),
            } => {
                data.first.dpi = cell_dimensions.dpi;
                data.second.dpi = cell_dimensions.dpi;
                match data.direction {
                    SplitDirection::Horizontal => {
                        let new_rows = (data.first.rows as isize)
                            .saturating_add(y_adjust)
                            .max(min_y as isize);
                        y_adjust = new_rows.saturating_sub(data.first.rows as isize);

                        if y_adjust != 0 {
                            adjust_y_size(&mut *left, y_adjust, cell_dimensions);
                            data.first.rows = new_rows.try_into().unwrap();
                            data.first.pixel_height =
                                data.first.rows.saturating_mul(cell_dimensions.pixel_height);

                            adjust_y_size(&mut *right, y_adjust, cell_dimensions);
                            data.second.rows = data.first.rows;
                            data.second.pixel_height = data.first.pixel_height;
                        }
                        return;
                    }
                    SplitDirection::Vertical if y_adjust > 0 => {
                        adjust_y_size(&mut *left, 1, cell_dimensions);
                        data.first.rows += 1;
                        data.first.pixel_height =
                            data.first.rows.saturating_mul(cell_dimensions.pixel_height);
                        y_adjust -= 1;
                        if y_adjust > 0 {
                            adjust_y_size(&mut *right, 1, cell_dimensions);
                            data.second.rows += 1;
                            data.second.pixel_height = data
                                .second
                                .rows
                                .saturating_mul(cell_dimensions.pixel_height);
                            y_adjust -= 1;
                        }
                    }
                    SplitDirection::Vertical => {
                        // y_adjust is negative
                        if data.first.rows > 1 {
                            adjust_y_size(&mut *left, -1, cell_dimensions);
                            data.first.rows -= 1;
                            data.first.pixel_height =
                                data.first.rows.saturating_mul(cell_dimensions.pixel_height);
                            y_adjust += 1;
                        }
                        if y_adjust < 0 && data.second.rows > 1 {
                            adjust_y_size(&mut *right, -1, cell_dimensions);
                            data.second.rows -= 1;
                            data.second.pixel_height = data
                                .second
                                .rows
                                .saturating_mul(cell_dimensions.pixel_height);
                            y_adjust += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Keep at least one terminal row when a window or split becomes very small.
fn pane_content_size(
    mut size: TerminalSize,
    header_rows: usize,
    padding: (usize, usize),
) -> TerminalSize {
    let cell_width = size.pixel_width.checked_div(size.cols).unwrap_or(0);
    let cell_height = size.pixel_height.checked_div(size.rows).unwrap_or(0);
    let inset = header_rows.min(size.rows.saturating_sub(1));
    let available_width = size.pixel_width.saturating_sub(padding.0.saturating_mul(2));
    let available_height = size
        .pixel_height
        .saturating_sub(inset.saturating_mul(cell_height))
        .saturating_sub(padding.1.saturating_mul(2));
    size.cols = (available_width / cell_width.max(1)).max(1);
    size.rows = (available_height / cell_height.max(1)).max(1);
    size.pixel_width = size.cols * cell_width;
    size.pixel_height = size.rows * cell_height;
    size
}

/// Font overrides affect only the PTY grid. Split geometry and header height
/// continue to use the tab's global layout grid.
fn pane_size_with_cell(
    size: TerminalSize,
    header_rows: usize,
    padding: (usize, usize),
    cell: Option<(usize, usize)>,
) -> TerminalSize {
    let mut size = pane_content_size(size, header_rows, padding);
    if let Some((cell_width, cell_height)) = cell {
        size.cols = (size.pixel_width / cell_width).max(1);
        size.rows = (size.pixel_height / cell_height).max(1);
        size.pixel_width = size.cols * cell_width;
        size.pixel_height = size.rows * cell_height;
    }
    size
}

fn apply_sizes_from_splits(
    tree: &Tree,
    size: &TerminalSize,
    header_rows: usize,
    padding: (usize, usize),
    cell_sizes: &HashMap<PaneId, (usize, usize)>,
) {
    match tree {
        Tree::Empty => return,
        Tree::Node { data: None, .. } => return,
        Tree::Node {
            left,
            right,
            data: Some(data),
        } => {
            apply_sizes_from_splits(&*left, &data.first, header_rows, padding, cell_sizes);
            apply_sizes_from_splits(&*right, &data.second, header_rows, padding, cell_sizes);
        }
        Tree::Leaf(pane) => {
            pane.resize(pane_size_with_cell(
                *size,
                header_rows,
                padding,
                cell_sizes.get(&pane.pane_id()).copied(),
            ))
            .ok();
        }
    }
}

fn cell_dimensions(size: &TerminalSize) -> TerminalSize {
    TerminalSize {
        rows: 1,
        cols: 1,
        pixel_width: size.pixel_width / size.cols,
        pixel_height: size.pixel_height / size.rows,
        dpi: size.dpi,
    }
}

impl Tab {
    pub fn new(size: &TerminalSize) -> Self {
        let inner = TabInner::new(size);
        let tab_id = inner.id;
        Self {
            inner: Mutex::new(inner),
            tab_id,
        }
    }

    pub fn get_title(&self) -> String {
        self.inner.lock().title.clone()
    }

    pub fn set_title(&self, title: &str) {
        let mut inner = self.inner.lock();
        if inner.title != title {
            inner.title = title.to_string();
            Mux::try_get().map(|mux| {
                mux.notify(MuxNotification::TabTitleChanged {
                    tab_id: inner.id,
                    title: title.to_string(),
                })
            });
        }
    }

    /// Called by the multiplexer client when building a local tab to
    /// mirror a remote tab.  The supplied `root` is the information
    /// about our counterpart in the remote server.
    /// This method builds a local tree based on the remote tree which
    /// then replaces the local tree structure.
    ///
    /// The `make_pane` function is provided by the caller, and its purpose
    /// is to lookup an existing Pane that corresponds to the provided
    /// PaneEntry, or to create a new Pane from that entry.
    /// make_pane is expected to add the pane to the mux if it creates
    /// a new pane, otherwise the pane won't poll/update in the GUI.
    pub fn sync_with_pane_tree<F>(&self, size: TerminalSize, root: PaneNode, make_pane: F)
    where
        F: FnMut(PaneEntry) -> Arc<dyn Pane>,
    {
        self.inner.lock().sync_with_pane_tree(size, root, make_pane)
    }

    pub fn codec_pane_tree(&self) -> PaneNode {
        self.inner.lock().codec_pane_tree()
    }

    /// Returns a count of how many panes are in this tab
    pub fn count_panes(&self) -> Option<usize> {
        self.inner.try_lock().map(|mut inner| inner.count_panes())
    }

    /// Sets the zoom state, returns the prior state
    pub fn set_zoomed(&self, zoomed: bool) -> bool {
        self.inner.lock().set_zoomed(zoomed)
    }

    pub fn toggle_zoom(&self) {
        self.inner.lock().toggle_zoom()
    }

    pub fn contains_pane(&self, pane: PaneId) -> bool {
        self.inner.lock().contains_pane(pane)
    }

    pub fn iter_panes(&self) -> Vec<PositionedPane> {
        self.inner.lock().iter_panes()
    }

    pub fn iter_panes_ignoring_zoom(&self) -> Vec<PositionedPane> {
        self.inner.lock().iter_panes_ignoring_zoom()
    }

    pub fn rotate_counter_clockwise(&self) {
        self.inner.lock().rotate_counter_clockwise()
    }

    pub fn rotate_clockwise(&self) {
        self.inner.lock().rotate_clockwise()
    }

    pub fn iter_splits(&self) -> Vec<PositionedSplit> {
        self.inner.lock().iter_splits()
    }

    pub fn tab_id(&self) -> TabId {
        self.tab_id
    }

    pub fn get_size(&self) -> TerminalSize {
        self.inner.lock().get_size()
    }

    /// Reserve cell-aligned chrome above each pane while keeping split positions
    /// in outer coordinates. Changing the inset resizes the PTYs once.
    pub fn set_pane_header_rows(&self, rows: usize) {
        let mut inner = self.inner.lock();
        if inner.pane_header_rows != rows {
            inner.pane_header_rows = rows;
            let size = inner.size;
            inner.resize(size);
        }
    }

    /// Reserve symmetric physical-pixel padding around each pane's terminal
    /// grid. This only changes the PTY size; split geometry stays unchanged.
    pub fn set_pane_content_padding(&self, padding: (usize, usize)) {
        let mut inner = self.inner.lock();
        if inner.pane_content_padding != padding {
            inner.pane_content_padding = padding;
            let size = inner.size;
            inner.resize(size);
        }
    }

    /// Update pane chrome in one resize so GUI layout synchronization cannot
    /// expose an intermediate header-only or padding-only terminal size.
    pub fn set_pane_chrome(&self, header_rows: usize, padding: (usize, usize)) {
        let mut inner = self.inner.lock();
        if inner.pane_header_rows != header_rows || inner.pane_content_padding != padding {
            inner.pane_header_rows = header_rows;
            inner.pane_content_padding = padding;
            let size = inner.size;
            inner.resize(size);
        }
    }

    pub fn pane_header_rows(&self) -> usize {
        self.inner.lock().pane_header_rows
    }

    pub fn pane_content_padding(&self) -> (usize, usize) {
        self.inner.lock().pane_content_padding
    }

    /// Override this pane's terminal cell size in physical pixels for the
    /// current session. This does not change the split's outer layout grid.
    /// Reapplying an unchanged size is cheap and does not resize the PTY.
    pub fn set_pane_cell_size(&self, pane_id: PaneId, cell: Option<(usize, usize)>) {
        let mut inner = self.inner.lock();
        if cell.map_or(false, |(width, height)| width == 0 || height == 0)
            || inner.pane_cell_sizes.get(&pane_id).copied() == cell
            || !inner.contains_pane(pane_id)
        {
            return;
        }
        match cell {
            Some(cell) => {
                inner.pane_cell_sizes.insert(pane_id, cell);
            }
            None => {
                inner.pane_cell_sizes.remove(&pane_id);
            }
        }
        if let Some(position) = inner
            .iter_panes()
            .into_iter()
            .find(|p| p.pane.pane_id() == pane_id)
        {
            let size = TerminalSize {
                cols: position.width,
                rows: position.height,
                pixel_width: position.pixel_width,
                pixel_height: position.pixel_height,
                dpi: inner.size.dpi,
            };
            position
                .pane
                .resize(inner.pane_content_size(pane_id, size))
                .ok();
        }
        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(inner.id)));
    }

    /// Let native GUI tabs retain divider proportions when their available
    /// space changes. Leave disabled for layouts owned by a remote mux/tmux.
    pub fn set_proportional_resize(&self, enabled: bool) {
        let mut inner = self.inner.lock();
        if inner.proportional_resize != enabled {
            inner.proportional_resize = enabled;
            inner.resize_reference = None;
        }
    }

    /// Apply the new size of the tab to the panes contained within.
    /// The delta between the current and the new size is computed,
    /// and is distributed between the splits.  For small resizes
    /// this algorithm biases towards adjusting the left/top nodes
    /// first. Native GUI tabs can opt into stable proportional resizing via
    /// `set_proportional_resize`.
    pub fn resize(&self, size: TerminalSize) {
        let mut inner = self.inner.lock();
        // Animated native chrome can request the same cell-aligned size on
        // adjacent frames. Avoid unnecessary PTY resizes and resize events.
        // Internal calls still force a refresh after a split/header change.
        if inner.proportional_resize && inner.size == size {
            return;
        }
        inner.resize(size)
    }

    /// Called when running in the mux server after an individual pane
    /// has been resized.
    /// Because the split manipulation happened on the GUI we "lost"
    /// the information that would have allowed us to call resize_split_by()
    /// and instead need to back-infer the split size information.
    /// We rely on the client to have resized (or be in the process
    /// of resizing) affected panes consistently with its own Tab
    /// tree model.
    /// This method does a simple tree walk to the leaves to back-propagate
    /// the size of the panes up to their containing node split data.
    /// Without this step, disconnecting and reconnecting would cause
    /// the GUI to use stale size information for the window it spawns
    /// to attach this tab.
    pub fn rebuild_splits_sizes_from_contained_panes(&self) {
        self.inner
            .lock()
            .rebuild_splits_sizes_from_contained_panes()
    }

    /// Given split_index, the topological index of a split returned by
    /// iter_splits() as PositionedSplit::index, revised the split position
    /// by the provided delta; positive values move the split to the right/bottom,
    /// and negative values to the left/top.
    /// The adjusted size is propogated downwards to contained children and
    /// their panes are resized accordingly.
    pub fn resize_split_by(&self, split_index: usize, delta: isize) {
        self.inner.lock().resize_split_by(split_index, delta)
    }

    /// Adjusts the size of the active pane in the specified direction
    /// by the specified amount.
    pub fn adjust_pane_size(&self, direction: PaneDirection, amount: usize) {
        self.inner.lock().adjust_pane_size(direction, amount)
    }

    /// Activate an adjacent pane in the specified direction.
    /// In cases where there are multiple adjacent panes in the
    /// intended direction, we take the pane that has the largest
    /// edge intersection.
    pub fn activate_pane_direction(&self, direction: PaneDirection) {
        self.inner.lock().activate_pane_direction(direction)
    }

    /// Returns an adjacent pane in the specified direction.
    /// In cases where there are multiple adjacent panes in the
    /// intended direction, we take the pane that has the largest
    /// edge intersection.
    pub fn get_pane_direction(&self, direction: PaneDirection, ignore_zoom: bool) -> Option<usize> {
        self.inner.lock().get_pane_direction(direction, ignore_zoom)
    }

    pub fn prune_dead_panes(&self) -> bool {
        self.inner.lock().prune_dead_panes()
    }

    pub fn kill_pane(&self, pane_id: PaneId) -> bool {
        self.inner.lock().kill_pane(pane_id)
    }

    pub fn kill_panes_in_domain(&self, domain: DomainId) -> bool {
        self.inner.lock().kill_panes_in_domain(domain)
    }

    /// Remove pane from tab.
    /// The pane is still live in the mux; the intent is for the pane to
    /// be added to a different tab.
    pub fn remove_pane(&self, pane_id: PaneId) -> Option<Arc<dyn Pane>> {
        self.inner.lock().remove_pane(pane_id)
    }

    pub fn can_close_without_prompting(&self, reason: CloseReason) -> bool {
        self.inner.lock().can_close_without_prompting(reason)
    }

    pub fn is_dead(&self) -> bool {
        self.inner.lock().is_dead()
    }

    pub fn get_active_pane(&self) -> Option<Arc<dyn Pane>> {
        self.inner.lock().get_active_pane()
    }

    #[allow(unused)]
    pub fn get_active_idx(&self) -> usize {
        self.inner.lock().get_active_idx()
    }

    pub fn set_active_pane(&self, pane: &Arc<dyn Pane>) {
        self.inner.lock().set_active_pane(pane)
    }

    pub fn set_active_idx(&self, pane_index: usize) {
        self.inner.lock().set_active_idx(pane_index)
    }

    /// Assigns the root pane.
    /// This is suitable when creating a new tab and then assigning
    /// the initial pane
    pub fn assign_pane(&self, pane: &Arc<dyn Pane>) {
        self.inner.lock().assign_pane(pane)
    }

    /// Swap the active pane with the specified pane_index
    pub fn swap_active_with_index(&self, pane_index: usize, keep_focus: bool) -> Option<()> {
        self.inner
            .lock()
            .swap_active_with_index(pane_index, keep_focus)
    }

    /// Computes the size of the pane that would result if the specified
    /// pane was split in a particular direction.
    /// The intent is to call this prior to spawning the new pane so that
    /// you can create it with the correct size.
    /// May return None if the specified pane_index is invalid.
    pub fn compute_split_size(
        &self,
        pane_index: usize,
        request: SplitRequest,
    ) -> Option<SplitDirectionAndSize> {
        self.inner.lock().compute_split_size(pane_index, request)
    }

    /// Split the pane that has pane_index in the given direction and assign
    /// the right/bottom pane of the newly created split to the provided Pane
    /// instance.  Returns the resultant index of the newly inserted pane.
    /// Both the split and the inserted pane will be resized.
    pub fn split_and_insert(
        &self,
        pane_index: usize,
        request: SplitRequest,
        pane: Arc<dyn Pane>,
    ) -> anyhow::Result<usize> {
        self.inner
            .lock()
            .split_and_insert(pane_index, request, pane)
    }

    /// Keep the source owned by its tab until the destination accepts it.
    /// Lock ordering also makes this safe when two callers move in opposite directions.
    pub(crate) fn move_pane_from(
        &self,
        source: &Tab,
        source_id: PaneId,
        target_id: PaneId,
        request: SplitRequest,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        anyhow::ensure!(source_id != target_id, "cannot split a pane into itself");
        if self.tab_id() == source.tab_id() {
            return self
                .inner
                .lock()
                .move_pane_within_tab(source_id, target_id, request);
        }
        let (mut source_inner, mut target_inner) = if source.tab_id() < self.tab_id() {
            let source_inner = source.inner.lock();
            (source_inner, self.inner.lock())
        } else {
            let target_inner = self.inner.lock();
            (source.inner.lock(), target_inner)
        };
        let source_pos = source_inner
            .iter_panes_ignoring_zoom()
            .into_iter()
            .find(|p| p.pane.pane_id() == source_id)
            .ok_or_else(|| anyhow::anyhow!("source pane {} is no longer in its tab", source_id))?;
        let target_pos = target_inner
            .iter_panes_ignoring_zoom()
            .into_iter()
            .find(|p| p.pane.pane_id() == target_id)
            .ok_or_else(|| anyhow::anyhow!("target pane {} is no longer in its tab", target_id))?;
        let original_source_size =
            if source_inner.zoomed.as_ref().map(|p| p.pane_id()) == Some(source_id) {
                source_inner.size
            } else {
                TerminalSize {
                    rows: source_pos.height,
                    cols: source_pos.width,
                    pixel_width: source_pos.pixel_width,
                    pixel_height: source_pos.pixel_height,
                    dpi: source_inner.size.dpi,
                }
            };
        let pane = source_pos.pane;
        let original_target_cell = target_inner.pane_cell_sizes.remove(&source_id);
        if let Some(cell) = source_inner.pane_cell_sizes.get(&source_id).copied() {
            target_inner.pane_cell_sizes.insert(source_id, cell);
        }
        if let Err(error) =
            target_inner.split_and_insert(target_pos.index, request, Arc::clone(&pane))
        {
            // A successful first resize may precede a failed second resize.
            // Preserve ownership even if the disconnected pty rejects rollback.
            target_inner.pane_cell_sizes.remove(&source_id);
            if let Some(cell) = original_target_cell {
                target_inner.pane_cell_sizes.insert(source_id, cell);
            }
            pane.resize(source_inner.pane_content_size(source_id, original_source_size))
                .ok();
            return Err(error);
        }
        source_inner.remove_pane(source_id);
        Ok(pane)
    }

    pub fn get_zoomed_pane(&self) -> Option<Arc<dyn Pane>> {
        self.inner.lock().get_zoomed_pane()
    }
}

impl TabInner {
    fn move_pane_within_tab(
        &mut self,
        source_id: PaneId,
        target_id: PaneId,
        request: SplitRequest,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        anyhow::ensure!(
            self.contains_pane(source_id),
            "invalid source pane {}",
            source_id
        );
        anyhow::ensure!(
            self.contains_pane(target_id),
            "invalid target pane {}",
            target_id
        );
        let original_tree = clone_pane_tree(self.pane.as_ref().unwrap());
        let original_size = self.size;
        let original_active = self.active;
        let original_recency = self.recency.clone();
        let original_zoomed = self.zoomed.clone();
        let original_size_before_zoom = self.size_before_zoom;
        let original_resize_reference = self.resize_reference.clone();
        let source_cell = self.pane_cell_sizes.get(&source_id).copied();
        let pane = self.remove_pane(source_id).unwrap();
        if let Some(cell) = source_cell {
            self.pane_cell_sizes.insert(source_id, cell);
        }
        let target_index = self
            .iter_panes_ignoring_zoom()
            .iter()
            .find(|p| p.pane.pane_id() == target_id)
            .unwrap()
            .index;
        if let Err(error) = self.split_and_insert(target_index, request, Arc::clone(&pane)) {
            self.pane = Some(original_tree);
            self.size = original_size;
            self.active = original_active;
            self.recency = original_recency;
            self.zoomed = original_zoomed;
            self.size_before_zoom = original_size_before_zoom;
            self.resize_reference = original_resize_reference;
            apply_sizes_from_splits(
                self.pane.as_ref().unwrap(),
                &original_size,
                self.pane_header_rows,
                self.pane_content_padding,
                &self.pane_cell_sizes,
            );
            if let Some(zoomed) = &self.zoomed {
                zoomed
                    .resize(self.pane_content_size(zoomed.pane_id(), original_size))
                    .ok();
            }
            return Err(error);
        }
        Ok(pane)
    }

    fn new(size: &TerminalSize) -> Self {
        Self {
            id: TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            pane: Some(Tree::new()),
            size: *size,
            size_before_zoom: *size,
            pane_header_rows: 0,
            pane_content_padding: (0, 0),
            proportional_resize: false,
            resize_reference: None,
            pane_cell_sizes: HashMap::new(),
            active: 0,
            zoomed: None,
            title: String::new(),
            recency: Recency::default(),
        }
    }

    fn sync_with_pane_tree<F>(&mut self, size: TerminalSize, root: PaneNode, mut make_pane: F)
    where
        F: FnMut(PaneEntry) -> Arc<dyn Pane>,
    {
        let mut active = None;
        let mut zoomed = None;

        log::debug!("sync_with_pane_tree with size {:?}", size);

        let t = build_from_pane_tree(root.into_tree(), &mut active, &mut zoomed, &mut make_pane);
        let mut cursor = t.cursor();

        self.active = 0;
        if let Some(active) = active {
            // Resolve the active pane to its index
            let mut index = 0;
            loop {
                if let Some(pane) = cursor.leaf_mut() {
                    if active.pane_id() == pane.pane_id() {
                        // Found it
                        self.active = index;
                        self.recency.tag(index);
                        break;
                    }
                    index += 1;
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(c) => {
                        // Didn't find it
                        cursor = c;
                        break;
                    }
                }
            }
        }
        self.pane.replace(cursor.tree());
        self.zoomed = zoomed;
        self.size = size;
        self.resize_reference = None;

        self.resize(size);

        log::debug!(
            "sync tab: {:#?} zoomed: {} {:#?}",
            size,
            self.zoomed.is_some(),
            self.iter_panes()
        );
        assert!(self.pane.is_some());
    }

    fn codec_pane_tree(&mut self) -> PaneNode {
        let mux = Mux::get();
        let tab_id = self.id;
        let window_id = match mux.window_containing_tab(tab_id) {
            Some(w) => w,
            None => {
                log::error!("no window contains tab {}", tab_id);
                return PaneNode::Empty;
            }
        };

        let workspace = match mux
            .get_window(window_id)
            .map(|w| w.get_workspace().to_string())
        {
            Some(ws) => ws,
            None => {
                log::error!("window id {} doesn't have a window!?", window_id);
                return PaneNode::Empty;
            }
        };

        let active = self.get_active_pane();
        let zoomed = self.zoomed.as_ref();
        if let Some(root) = self.pane.as_ref() {
            pane_tree(
                root,
                tab_id,
                window_id,
                active.as_ref(),
                zoomed,
                &workspace,
                0,
                0,
            )
        } else {
            PaneNode::Empty
        }
    }

    /// Returns a count of how many panes are in this tab
    fn count_panes(&mut self) -> usize {
        let mut count = 0;
        let mut cursor = self.pane.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                count += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    return count;
                }
            }
        }
    }

    /// Sets the zoom state, returns the prior state
    fn set_zoomed(&mut self, zoomed: bool) -> bool {
        if self.zoomed.is_some() == zoomed {
            // Current zoom state matches intended zoom state,
            // so we have nothing to do.
            return zoomed;
        }
        self.toggle_zoom();
        !zoomed
    }

    fn toggle_zoom(&mut self) {
        let size = self.size;
        if self.zoomed.take().is_some() {
            // We were zoomed, but now we are not.
            // Re-apply the size to the panes
            if let Some(pane) = self.get_active_pane() {
                pane.set_zoomed(false);
            }
            self.size = self.size_before_zoom;
            self.resize(size);
        } else {
            // We weren't zoomed, but now we want to zoom.
            // Locate the active pane
            self.size_before_zoom = size;
            if let Some(pane) = self.get_active_pane() {
                pane.set_zoomed(true);
                pane.resize(self.pane_content_size(pane.pane_id(), size))
                    .ok();
                self.zoomed.replace(pane);
            }
        }
        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
    }

    fn contains_pane(&self, pane: PaneId) -> bool {
        fn contains(tree: &Tree, pane: PaneId) -> bool {
            match tree {
                Tree::Empty => false,
                Tree::Node { left, right, .. } => contains(left, pane) || contains(right, pane),
                Tree::Leaf(p) => p.pane_id() == pane,
            }
        }
        match &self.pane {
            Some(root) => contains(root, pane),
            None => false,
        }
    }

    /// Walks the pane tree to produce the topologically ordered flattened
    /// list of PositionedPane instances along with their positioning information.
    fn iter_panes(&mut self) -> Vec<PositionedPane> {
        self.iter_panes_impl(true)
    }

    /// Like iter_panes, except that it will include all panes, regardless of
    /// whether one of them is currently zoomed.
    fn iter_panes_ignoring_zoom(&mut self) -> Vec<PositionedPane> {
        self.iter_panes_impl(false)
    }

    fn rotate_counter_clockwise(&mut self) {
        let panes = self.iter_panes_ignoring_zoom();
        if panes.is_empty() {
            // Shouldn't happen, but we check for this here so that the
            // expect below cannot trigger a panic
            return;
        }
        let mut pane_to_swap = panes
            .first()
            .map(|p| p.pane.clone())
            .expect("at least one pane");

        let mut cursor = self.pane.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                std::mem::swap(&mut pane_to_swap, cursor.leaf_mut().unwrap());
            }

            match cursor.postorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    let size = self.size;
                    apply_sizes_from_splits(
                        self.pane.as_mut().unwrap(),
                        &size,
                        self.pane_header_rows,
                        self.pane_content_padding,
                        &self.pane_cell_sizes,
                    );
                    break;
                }
            }
        }
    }

    fn rotate_clockwise(&mut self) {
        let panes = self.iter_panes_ignoring_zoom();
        if panes.is_empty() {
            // Shouldn't happen, but we check for this here so that the
            // expect below cannot trigger a panic
            return;
        }
        let mut pane_to_swap = panes
            .last()
            .map(|p| p.pane.clone())
            .expect("at least one pane");

        let mut cursor = self.pane.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                std::mem::swap(&mut pane_to_swap, cursor.leaf_mut().unwrap());
            }

            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    let size = self.size;
                    apply_sizes_from_splits(
                        self.pane.as_mut().unwrap(),
                        &size,
                        self.pane_header_rows,
                        self.pane_content_padding,
                        &self.pane_cell_sizes,
                    );
                    break;
                }
            }
        }
        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
    }

    fn iter_panes_impl(&mut self, respect_zoom_state: bool) -> Vec<PositionedPane> {
        let mut panes = vec![];

        if respect_zoom_state {
            if let Some(zoomed) = self.zoomed.as_ref() {
                let size = self.size;
                panes.push(PositionedPane {
                    index: 0,
                    is_active: true,
                    is_zoomed: true,
                    left: 0,
                    top: 0,
                    width: size.cols.into(),
                    pixel_width: size.pixel_width.into(),
                    height: size.rows.into(),
                    pixel_height: size.pixel_height.into(),
                    pane: Arc::clone(zoomed),
                });
                return panes;
            }
        }

        let active_idx = self.active;
        let zoomed_id = self.zoomed.as_ref().map(|p| p.pane_id());
        let root_size = self.size;
        let mut cursor = self.pane.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                let index = panes.len();
                let mut left = 0usize;
                let mut top = 0usize;
                let mut parent_size = None;
                for (branch, node) in cursor.path_to_root() {
                    if let Some(node) = node {
                        if parent_size.is_none() {
                            parent_size.replace(if branch == PathBranch::IsRight {
                                node.second
                            } else {
                                node.first
                            });
                        }
                        if branch == PathBranch::IsRight {
                            top += node.top_of_second();
                            left += node.left_of_second();
                        }
                    }
                }

                let pane = Arc::clone(cursor.leaf_mut().unwrap());
                let dims = parent_size.unwrap_or_else(|| root_size);

                panes.push(PositionedPane {
                    index,
                    is_active: index == active_idx,
                    is_zoomed: zoomed_id == Some(pane.pane_id()),
                    left,
                    top,
                    width: dims.cols as _,
                    height: dims.rows as _,
                    pixel_width: dims.pixel_width as _,
                    pixel_height: dims.pixel_height as _,
                    pane,
                });
            }

            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    break;
                }
            }
        }

        panes
    }

    fn iter_splits(&mut self) -> Vec<PositionedSplit> {
        let mut dividers = vec![];
        if self.zoomed.is_some() {
            return dividers;
        }

        let mut cursor = self.pane.take().unwrap().cursor();
        let mut index = 0;

        loop {
            if !cursor.is_leaf() {
                let mut left = 0usize;
                let mut top = 0usize;
                for (branch, p) in cursor.path_to_root() {
                    if let Some(p) = p {
                        if branch == PathBranch::IsRight {
                            left += p.left_of_second();
                            top += p.top_of_second();
                        }
                    }
                }
                if let Ok(Some(node)) = cursor.node_mut() {
                    match node.direction {
                        SplitDirection::Horizontal => left += node.first.cols as usize,
                        SplitDirection::Vertical => top += node.first.rows as usize,
                    }

                    dividers.push(PositionedSplit {
                        index,
                        direction: node.direction,
                        left,
                        top,
                        size: if node.direction == SplitDirection::Horizontal {
                            node.height() as usize
                        } else {
                            node.width() as usize
                        },
                    })
                }
                index += 1;
            }

            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    break;
                }
            }
        }

        dividers
    }

    fn get_size(&self) -> TerminalSize {
        self.size
    }

    fn pane_content_size(&self, pane_id: PaneId, size: TerminalSize) -> TerminalSize {
        pane_size_with_cell(
            size,
            self.pane_header_rows,
            self.pane_content_padding,
            self.pane_cell_sizes.get(&pane_id).copied(),
        )
    }

    fn prepare_resize_reference(&mut self) {
        if self.proportional_resize {
            let tree = self.pane.as_ref().unwrap();
            if !self
                .resize_reference
                .as_ref()
                .map_or(false, |reference| reference.matches(tree))
            {
                self.resize_reference = Some(SplitResizeReference::capture(tree));
            }
        }
    }

    fn resize(&mut self, size: TerminalSize) {
        if size.rows == 0 || size.cols == 0 {
            // Ignore "impossible" resize requests
            return;
        }

        if let Some(zoomed) = &self.zoomed {
            self.size = size;
            zoomed
                .resize(self.pane_content_size(zoomed.pane_id(), size))
                .ok();
        } else {
            let dims = cell_dimensions(&size);
            let (min_x, min_y) = compute_min_size(self.pane.as_mut().unwrap());
            let current_size = self.size;

            // Constrain the new size to the minimum possible dimensions
            let cols = size.cols.max(min_x);
            let rows = size.rows.max(min_y);
            let size = TerminalSize {
                rows,
                cols,
                pixel_width: cols * dims.pixel_width,
                pixel_height: rows * dims.pixel_height,
                dpi: dims.dpi,
            };

            // Update every split from the same stable layout reference; do not
            // feed the previous frame's integer rounding back into the ratios.
            if self.proportional_resize {
                self.prepare_resize_reference();
                self.resize_reference
                    .as_ref()
                    .unwrap()
                    .apply(self.pane.as_mut().unwrap(), size);
            } else {
                adjust_x_size(
                    self.pane.as_mut().unwrap(),
                    cols as isize - current_size.cols as isize,
                    &dims,
                );
                adjust_y_size(
                    self.pane.as_mut().unwrap(),
                    rows as isize - current_size.rows as isize,
                    &dims,
                );
            }

            self.size = size;

            // And then resize the individual panes to match
            apply_sizes_from_splits(
                self.pane.as_mut().unwrap(),
                &size,
                self.pane_header_rows,
                self.pane_content_padding,
                &self.pane_cell_sizes,
            );
        }

        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
    }

    fn apply_pane_size(&mut self, pane_size: TerminalSize, cursor: &mut Cursor) {
        if self.proportional_resize {
            let (minimum_first, minimum_second) = match cursor.subtree() {
                Tree::Node { left, right, .. } => (compute_min_size(left), compute_min_size(right)),
                _ => return,
            };
            if let Ok(Some(node)) = cursor.node_mut() {
                let (first, second) = split_dimensions(node);
                resize_split_proportionally(
                    node,
                    pane_size,
                    first,
                    second,
                    minimum_first,
                    minimum_second,
                );
            }
            return;
        }
        let cell_width = pane_size
            .pixel_width
            .checked_div(pane_size.cols)
            .unwrap_or(1);
        let cell_height = pane_size
            .pixel_height
            .checked_div(pane_size.rows)
            .unwrap_or(1);
        if let Ok(Some(node)) = cursor.node_mut() {
            // Adjust the size of the node; we preserve the size of the first
            // child and adjust the second, so if we are split down the middle
            // and the window is made wider, the right column will grow in
            // size, leaving the left at its current width.
            if node.direction == SplitDirection::Horizontal {
                node.first.rows = pane_size.rows;
                node.second.rows = pane_size.rows;

                node.second.cols = pane_size.cols.saturating_sub(1 + node.first.cols);
            } else {
                node.first.cols = pane_size.cols;
                node.second.cols = pane_size.cols;

                node.second.rows = pane_size.rows.saturating_sub(1 + node.first.rows);
            }
            node.first.pixel_width = node.first.cols * cell_width;
            node.first.pixel_height = node.first.rows * cell_height;

            node.second.pixel_width = node.second.cols * cell_width;
            node.second.pixel_height = node.second.rows * cell_height;
        }
    }

    fn rebuild_splits_sizes_from_contained_panes(&mut self) {
        if self.zoomed.is_some() {
            return;
        }

        fn compute_size(node: &mut Tree) -> Option<TerminalSize> {
            match node {
                Tree::Empty => None,
                Tree::Leaf(pane) => {
                    let dims = pane.get_dimensions();
                    let size = TerminalSize {
                        cols: dims.cols,
                        rows: dims.viewport_rows,
                        pixel_height: dims.pixel_height,
                        pixel_width: dims.pixel_width,
                        dpi: dims.dpi,
                    };
                    Some(size)
                }
                Tree::Node { left, right, data } => {
                    if let Some(data) = data {
                        if let Some(first) = compute_size(left) {
                            data.first = first;
                        }
                        if let Some(second) = compute_size(right) {
                            data.second = second;
                        }
                        Some(data.size())
                    } else {
                        None
                    }
                }
            }
        }

        if let Some(root) = self.pane.as_mut() {
            if let Some(size) = compute_size(root) {
                self.size = size;
            }
        }
        self.resize_reference = None;
        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
    }

    fn resize_split_by(&mut self, split_index: usize, delta: isize) {
        if self.zoomed.is_some() {
            return;
        }

        self.prepare_resize_reference();
        let mut cursor = self.pane.take().unwrap().cursor();
        let mut index = 0;

        // Position cursor on the specified split
        loop {
            if !cursor.is_leaf() {
                if index == split_index {
                    // Found it
                    break;
                }
                index += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    // Didn't find it
                    self.pane.replace(c.tree());
                    return;
                }
            }
        }

        // Now cursor is looking at the split
        if self.adjust_node_at_cursor(&mut cursor, delta) {
            self.cascade_size_from_cursor(cursor);
            Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
        } else {
            self.pane.replace(cursor.tree());
        }
    }

    fn adjust_node_at_cursor(&mut self, cursor: &mut Cursor, delta: isize) -> bool {
        let cell_dimensions = self.cell_dimensions();
        let (minimum_first, minimum_second) = if self.proportional_resize {
            match cursor.subtree() {
                Tree::Node { left, right, .. } => (compute_min_size(left), compute_min_size(right)),
                _ => ((1, 1), (1, 1)),
            }
        } else {
            ((1, 1), (1, 1))
        };
        if let Ok(Some(node)) = cursor.node_mut() {
            let original = *node;
            match node.direction {
                SplitDirection::Horizontal => {
                    let width = node.width();

                    let mut cols = node.first.cols as isize;
                    cols = cols
                        .saturating_add(delta)
                        .max(minimum_first.0 as isize)
                        .min((width as isize).saturating_sub(1 + minimum_second.0 as isize));
                    node.first.cols = cols as usize;
                    node.first.pixel_width =
                        node.first.cols.saturating_mul(cell_dimensions.pixel_width);

                    node.second.cols = width.saturating_sub(node.first.cols.saturating_add(1));
                    node.second.pixel_width =
                        node.second.cols.saturating_mul(cell_dimensions.pixel_width);
                }
                SplitDirection::Vertical => {
                    let height = node.height();

                    let mut rows = node.first.rows as isize;
                    rows = rows
                        .saturating_add(delta)
                        .max(minimum_first.1 as isize)
                        .min((height as isize).saturating_sub(1 + minimum_second.1 as isize));
                    node.first.rows = rows as usize;
                    node.first.pixel_height =
                        node.first.rows.saturating_mul(cell_dimensions.pixel_height);

                    node.second.rows = height.saturating_sub(node.first.rows.saturating_add(1));
                    node.second.pixel_height = node
                        .second
                        .rows
                        .saturating_mul(cell_dimensions.pixel_height);
                }
            }
            return *node != original;
        }
        false
    }

    fn cascade_size_from_cursor(&mut self, mut cursor: Cursor) {
        if self.proportional_resize {
            if let Some(reference) = self.resize_reference.as_mut() {
                reference.update_split(cursor.subtree());
            }
            self.pane.replace(cursor.tree());
            self.resize(self.size);
            return;
        }
        // Now we need to cascade this down to children
        match cursor.preorder_next() {
            Ok(c) => cursor = c,
            Err(c) => {
                self.pane.replace(c.tree());
                return;
            }
        }
        let root_size = self.size;

        loop {
            // Figure out the available size by looking at our immediate parent node.
            // If we are the root, look at the provided new size
            let pane_size = if let Some((branch, Some(parent))) = cursor.path_to_root().next() {
                if branch == PathBranch::IsRight {
                    parent.second
                } else {
                    parent.first
                }
            } else {
                root_size
            };

            if cursor.is_leaf() {
                // Apply our size to the tty
                cursor
                    .leaf_mut()
                    .map(|pane| pane.resize(self.pane_content_size(pane.pane_id(), pane_size)));
            } else {
                self.apply_pane_size(pane_size, &mut cursor);
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    break;
                }
            }
        }
        Mux::try_get().map(|mux| mux.notify(MuxNotification::TabResized(self.id)));
    }

    fn adjust_pane_size(&mut self, direction: PaneDirection, amount: usize) {
        if self.zoomed.is_some() {
            return;
        }
        self.prepare_resize_reference();
        let active_index = self.active;
        let mut cursor = self.pane.take().unwrap().cursor();
        let mut index = 0;

        // Position cursor on the active leaf
        loop {
            if cursor.is_leaf() {
                if index == active_index {
                    // Found it
                    break;
                }
                index += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    // Didn't find it
                    self.pane.replace(c.tree());
                    return;
                }
            }
        }

        // We are on the active leaf.
        // Now we go up until we find the parent node that is
        // aligned with the desired direction.
        let split_direction = match direction {
            PaneDirection::Left | PaneDirection::Right => SplitDirection::Horizontal,
            PaneDirection::Up | PaneDirection::Down => SplitDirection::Vertical,
            PaneDirection::Next | PaneDirection::Prev => unreachable!(),
        };
        let delta = match direction {
            PaneDirection::Down | PaneDirection::Right => amount as isize,
            PaneDirection::Up | PaneDirection::Left => -(amount as isize),
            PaneDirection::Next | PaneDirection::Prev => unreachable!(),
        };
        loop {
            match cursor.go_up() {
                Ok(mut c) => {
                    if let Ok(Some(node)) = c.node_mut() {
                        if node.direction == split_direction {
                            if self.adjust_node_at_cursor(&mut c, delta) {
                                self.cascade_size_from_cursor(c);
                            } else {
                                self.pane.replace(c.tree());
                            }
                            return;
                        }
                    }

                    cursor = c;
                }

                Err(c) => {
                    self.pane.replace(c.tree());
                    return;
                }
            }
        }
    }

    fn activate_pane_direction(&mut self, direction: PaneDirection) {
        if self.zoomed.is_some() {
            if !configuration().unzoom_on_switch_pane {
                return;
            }
            self.toggle_zoom();
        }
        if let Some(panel_idx) = self.get_pane_direction(direction, false) {
            self.set_active_idx(panel_idx);
        }
        let mux = Mux::get();
        if let Some(window_id) = mux.window_containing_tab(self.id) {
            mux.notify(MuxNotification::WindowInvalidated(window_id));
        }
    }

    fn get_pane_direction(&mut self, direction: PaneDirection, ignore_zoom: bool) -> Option<usize> {
        let panes = if ignore_zoom {
            self.iter_panes_ignoring_zoom()
        } else {
            self.iter_panes()
        };

        let active = match panes.iter().find(|pane| pane.is_active) {
            Some(p) => p,
            None => {
                // No active pane somehow...
                return Some(0);
            }
        };

        if matches!(direction, PaneDirection::Next | PaneDirection::Prev) {
            let max_pane_id = panes.iter().map(|p| p.index).max().unwrap_or(active.index);

            return Some(if direction == PaneDirection::Next {
                if active.index == max_pane_id {
                    0
                } else {
                    active.index + 1
                }
            } else {
                if active.index == 0 {
                    max_pane_id
                } else {
                    active.index - 1
                }
            });
        }

        let mut best = None;

        let recency = &self.recency;

        fn edge_intersects(
            active_start: usize,
            active_size: usize,
            current_start: usize,
            current_size: usize,
        ) -> bool {
            intersects_range(
                &(active_start..active_start + active_size),
                &(current_start..current_start + current_size),
            )
        }

        for pane in &panes {
            let score = match direction {
                PaneDirection::Right => {
                    if pane.left == active.left + active.width + 1
                        && edge_intersects(active.top, active.height, pane.top, pane.height)
                    {
                        1 + recency.score(pane.index)
                    } else {
                        0
                    }
                }
                PaneDirection::Left => {
                    if pane.left + pane.width + 1 == active.left
                        && edge_intersects(active.top, active.height, pane.top, pane.height)
                    {
                        1 + recency.score(pane.index)
                    } else {
                        0
                    }
                }
                PaneDirection::Up => {
                    if pane.top + pane.height + 1 == active.top
                        && edge_intersects(active.left, active.width, pane.left, pane.width)
                    {
                        1 + recency.score(pane.index)
                    } else {
                        0
                    }
                }
                PaneDirection::Down => {
                    if active.top + active.height + 1 == pane.top
                        && edge_intersects(active.left, active.width, pane.left, pane.width)
                    {
                        1 + recency.score(pane.index)
                    } else {
                        0
                    }
                }
                PaneDirection::Next | PaneDirection::Prev => unreachable!(),
            };

            if score > 0 {
                let target = match best.take() {
                    Some((best_score, best_pane)) if best_score > score => (best_score, best_pane),
                    _ => (score, pane),
                };
                best.replace(target);
            }
        }

        if let Some((_, target)) = best.take() {
            return Some(target.index);
        }
        None
    }

    fn prune_dead_panes(&mut self) -> bool {
        let mux = Mux::get();
        !self
            .remove_pane_if(
                |_, pane| {
                    // If the pane is no longer known to the mux, then its liveness
                    // state isn't guaranteed to be monitored or updated, so let's
                    // consider the pane effectively dead if it isn't in the mux.
                    // <https://github.com/wezterm/wezterm/issues/4030>
                    let in_mux = mux.get_pane(pane.pane_id()).is_some();
                    let dead = pane.is_dead();
                    log::trace!(
                        "prune_dead_panes: pane_id={} dead={} in_mux={}",
                        pane.pane_id(),
                        dead,
                        in_mux
                    );
                    dead || !in_mux
                },
                true,
            )
            .is_empty()
    }

    fn kill_pane(&mut self, pane_id: PaneId) -> bool {
        !self
            .remove_pane_if(|_, pane| pane.pane_id() == pane_id, true)
            .is_empty()
    }

    fn kill_panes_in_domain(&mut self, domain: DomainId) -> bool {
        !self
            .remove_pane_if(|_, pane| pane.domain_id() == domain, true)
            .is_empty()
    }

    fn remove_pane(&mut self, pane_id: PaneId) -> Option<Arc<dyn Pane>> {
        let panes = self.remove_pane_if(|_, pane| pane.pane_id() == pane_id, false);
        for pane in panes {
            return Some(pane);
        }
        None
    }

    fn remove_pane_if<F>(&mut self, f: F, kill: bool) -> Vec<Arc<dyn Pane>>
    where
        F: Fn(usize, &Arc<dyn Pane>) -> bool,
    {
        let mut dead_panes = vec![];
        let zoomed_pane = self.zoomed.as_ref().map(|p| p.pane_id());

        {
            let root_size = self.size;
            let mut cursor = self.pane.take().unwrap().cursor();
            let mut pane_index = 0;
            let mut removed_indices = vec![];
            let cell_dims = self.cell_dimensions();

            loop {
                // Figure out the available size by looking at our immediate parent node.
                // If we are the root, look at the tab size
                let pane_size = if let Some((branch, Some(parent))) = cursor.path_to_root().next() {
                    if branch == PathBranch::IsRight {
                        parent.second
                    } else {
                        parent.first
                    }
                } else {
                    root_size
                };

                if cursor.is_leaf() {
                    let pane = Arc::clone(cursor.leaf_mut().unwrap());
                    if f(pane_index, &pane) {
                        removed_indices.push(pane_index);
                        if Some(pane.pane_id()) == zoomed_pane {
                            // If we removed the zoomed pane, un-zoom our state!
                            self.zoomed.take();
                        }
                        let parent;
                        match cursor.unsplit_leaf() {
                            Ok((c, dead, p)) => {
                                dead_panes.push(dead);
                                parent = p.unwrap();
                                cursor = c;
                            }
                            Err(c) => {
                                // We might be the root, for example
                                if c.is_top() && c.is_leaf() {
                                    self.pane.replace(Tree::Empty);
                                    dead_panes.push(pane);
                                } else {
                                    self.pane.replace(c.tree());
                                }
                                break;
                            }
                        };

                        // Now we need to increase the size of the current node
                        // and propagate the revised size to its children.
                        let size = TerminalSize {
                            rows: parent.height(),
                            cols: parent.width(),
                            pixel_width: cell_dims.pixel_width * parent.width(),
                            pixel_height: cell_dims.pixel_height * parent.height(),
                            dpi: cell_dims.dpi,
                        };

                        if let Some(unsplit) = cursor.leaf_mut() {
                            unsplit
                                .resize(self.pane_content_size(unsplit.pane_id(), size))
                                .ok();
                        } else {
                            self.apply_pane_size(size, &mut cursor);
                        }
                    } else if !dead_panes.is_empty() {
                        // Apply our revised size to the tty
                        pane.resize(self.pane_content_size(pane.pane_id(), pane_size))
                            .ok();
                    }

                    pane_index += 1;
                } else if !dead_panes.is_empty() {
                    self.apply_pane_size(pane_size, &mut cursor);
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(c) => {
                        self.pane.replace(c.tree());
                        break;
                    }
                }
            }

            // Figure out which pane should now be active.
            // If panes earlier than the active pane were closed, then we
            // need to shift the active pane down
            let active_idx = self.active;
            removed_indices.retain(|&idx| idx <= active_idx);
            self.active = active_idx.saturating_sub(removed_indices.len());
        }

        if !dead_panes.is_empty() {
            self.resize_reference = None;
            for pane in &dead_panes {
                self.pane_cell_sizes.remove(&pane.pane_id());
            }
        }
        if !dead_panes.is_empty() && kill {
            let to_kill: Vec<_> = dead_panes.iter().map(|p| p.pane_id()).collect();
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get();
                for pane_id in to_kill.into_iter() {
                    mux.remove_pane(pane_id);
                }
            })
            .detach();
        }
        dead_panes
    }

    fn can_close_without_prompting(&mut self, reason: CloseReason) -> bool {
        let panes = self.iter_panes_ignoring_zoom();
        for pos in &panes {
            if !pos.pane.can_close_without_prompting(reason) {
                return false;
            }
        }
        true
    }

    fn is_dead(&mut self) -> bool {
        // Make sure we account for all panes, so that we don't
        // kill the whole tab if the zoomed pane is dead!
        let panes = self.iter_panes_ignoring_zoom();
        let mut dead_count = 0;
        for pos in &panes {
            if pos.pane.is_dead() {
                dead_count += 1;
            }
        }
        dead_count == panes.len()
    }

    fn get_active_pane(&mut self) -> Option<Arc<dyn Pane>> {
        if let Some(zoomed) = self.zoomed.as_ref() {
            return Some(Arc::clone(zoomed));
        }

        self.iter_panes_ignoring_zoom()
            .iter()
            .nth(self.active)
            .map(|p| Arc::clone(&p.pane))
    }

    fn get_active_idx(&self) -> usize {
        self.active
    }

    fn set_active_pane(&mut self, pane: &Arc<dyn Pane>) {
        let prior = self.get_active_pane();

        if is_pane(pane, &prior.as_ref()) {
            return;
        }

        if self.zoomed.is_some() {
            if !configuration().unzoom_on_switch_pane {
                return;
            }
            self.toggle_zoom();
        }

        if let Some(item) = self
            .iter_panes_ignoring_zoom()
            .iter()
            .find(|p| p.pane.pane_id() == pane.pane_id())
        {
            self.active = item.index;
            self.recency.tag(item.index);
            self.advise_focus_change(prior);
        }
    }

    fn advise_focus_change(&mut self, prior: Option<Arc<dyn Pane>>) {
        let mux = Mux::get();
        let current = self.get_active_pane();
        match (prior, current) {
            (Some(prior), Some(current)) if prior.pane_id() != current.pane_id() => {
                prior.focus_changed(false);
                current.focus_changed(true);
                mux.notify(MuxNotification::PaneFocused(current.pane_id()));
            }
            (None, Some(current)) => {
                current.focus_changed(true);
                mux.notify(MuxNotification::PaneFocused(current.pane_id()));
            }
            (Some(prior), None) => {
                prior.focus_changed(false);
            }
            (Some(_), Some(_)) | (None, None) => {
                // no change
            }
        }
    }

    fn set_active_idx(&mut self, pane_index: usize) {
        let prior = self.get_active_pane();
        self.active = pane_index;
        self.recency.tag(pane_index);
        self.advise_focus_change(prior);
    }

    fn assign_pane(&mut self, pane: &Arc<dyn Pane>) {
        if self.pane_header_rows != 0 {
            pane.resize(self.pane_content_size(pane.pane_id(), self.size))
                .ok();
        }
        match Tree::new().cursor().assign_top(Arc::clone(pane)) {
            Ok(c) => self.pane = Some(c.tree()),
            Err(_) => panic!("tried to assign root pane to non-empty tree"),
        }
    }

    fn cell_dimensions(&self) -> TerminalSize {
        cell_dimensions(&self.size)
    }

    fn swap_active_with_index(&mut self, pane_index: usize, keep_focus: bool) -> Option<()> {
        let active_idx = self.get_active_idx();
        let mut pane = self.get_active_pane()?;
        log::trace!(
            "swap_active_with_index: pane_index {} active {}",
            pane_index,
            active_idx
        );

        {
            let mut cursor = self.pane.take().unwrap().cursor();

            // locate the requested index
            match cursor.go_to_nth_leaf(pane_index) {
                Ok(c) => cursor = c,
                Err(c) => {
                    log::trace!("didn't find pane {pane_index}");
                    self.pane.replace(c.tree());
                    return None;
                }
            };

            std::mem::swap(&mut pane, cursor.leaf_mut().unwrap());

            // re-position to the root
            cursor = cursor.tree().cursor();

            // and now go and update the active idx
            match cursor.go_to_nth_leaf(active_idx) {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    log::trace!("didn't find active {active_idx}");
                    return None;
                }
            };

            std::mem::swap(&mut pane, cursor.leaf_mut().unwrap());
            self.pane.replace(cursor.tree());

            // Advise the panes of their new sizes
            let size = self.size;
            apply_sizes_from_splits(
                self.pane.as_mut().unwrap(),
                &size,
                self.pane_header_rows,
                self.pane_content_padding,
                &self.pane_cell_sizes,
            );
        }

        // And update focus
        if keep_focus {
            self.set_active_idx(pane_index);
        } else {
            self.advise_focus_change(Some(pane));
        }
        None
    }

    fn compute_split_size(
        &mut self,
        pane_index: usize,
        request: SplitRequest,
    ) -> Option<SplitDirectionAndSize> {
        let cell_dims = self.cell_dimensions();

        fn split_dimension(dim: usize, request: SplitRequest) -> (usize, usize) {
            let target_size = match request.size {
                SplitSize::Cells(n) => n,
                SplitSize::Percent(n) => (dim * (n as usize)) / 100,
            }
            .max(1);

            let remain = dim.saturating_sub(target_size + 1);

            if request.target_is_second {
                (remain, target_size)
            } else {
                (target_size, remain)
            }
        }

        if request.top_level {
            let size = self.size;

            let ((width1, width2), (height1, height2)) = match request.direction {
                SplitDirection::Horizontal => (
                    split_dimension(size.cols as usize, request),
                    (size.rows as usize, size.rows as usize),
                ),
                SplitDirection::Vertical => (
                    (size.cols as usize, size.cols as usize),
                    split_dimension(size.rows as usize, request),
                ),
            };

            return Some(SplitDirectionAndSize {
                direction: request.direction,
                first: TerminalSize {
                    rows: height1 as _,
                    cols: width1 as _,
                    pixel_height: cell_dims.pixel_height * height1,
                    pixel_width: cell_dims.pixel_width * width1,
                    dpi: cell_dims.dpi,
                },
                second: TerminalSize {
                    rows: height2 as _,
                    cols: width2 as _,
                    pixel_height: cell_dims.pixel_height * height2,
                    pixel_width: cell_dims.pixel_width * width2,
                    dpi: cell_dims.dpi,
                },
            });
        }

        // Ensure that we're not zoomed, otherwise we'll end up in
        // a bogus split state (https://github.com/wezterm/wezterm/issues/723)
        self.set_zoomed(false);

        self.iter_panes().iter().nth(pane_index).map(|pos| {
            let ((width1, width2), (height1, height2)) = match request.direction {
                SplitDirection::Horizontal => (
                    split_dimension(pos.width, request),
                    (pos.height, pos.height),
                ),
                SplitDirection::Vertical => {
                    ((pos.width, pos.width), split_dimension(pos.height, request))
                }
            };

            SplitDirectionAndSize {
                direction: request.direction,
                first: TerminalSize {
                    rows: height1 as _,
                    cols: width1 as _,
                    pixel_height: cell_dims.pixel_height * height1,
                    pixel_width: cell_dims.pixel_width * width1,
                    dpi: cell_dims.dpi,
                },
                second: TerminalSize {
                    rows: height2 as _,
                    cols: width2 as _,
                    pixel_height: cell_dims.pixel_height * height2,
                    pixel_width: cell_dims.pixel_width * width2,
                    dpi: cell_dims.dpi,
                },
            }
        })
    }

    fn split_and_insert(
        &mut self,
        pane_index: usize,
        request: SplitRequest,
        pane: Arc<dyn Pane>,
    ) -> anyhow::Result<usize> {
        if self.zoomed.is_some() {
            anyhow::bail!("cannot split while zoomed");
        }

        {
            let split_info = self
                .compute_split_size(pane_index, request)
                .ok_or_else(|| {
                    anyhow::anyhow!("invalid pane_index {}; cannot split!", pane_index)
                })?;

            let tab_size = self.size;
            if split_info.first.rows == 0
                || split_info.first.cols == 0
                || split_info.second.rows == 0
                || split_info.second.cols == 0
                || split_info.top_of_second() + split_info.second.rows > tab_size.rows
                || split_info.left_of_second() + split_info.second.cols > tab_size.cols
            {
                log::error!(
                    "No space for split!!! {:#?} height={} width={} top_of_second={} left_of_second={} tab_size={:?}",
                    split_info,
                    split_info.height(),
                    split_info.width(),
                    split_info.top_of_second(),
                    split_info.left_of_second(),
                    tab_size
                );
                anyhow::bail!("No space for split!");
            }

            let needs_resize = if request.top_level {
                self.pane.as_ref().unwrap().num_leaves() > 1
            } else {
                false
            };

            if needs_resize {
                // A new top-level leaf also needs the GUI inset. Resize it
                // before changing ownership so a PTY failure leaves the tree intact.
                let incoming_size = if request.target_is_second {
                    split_info.second
                } else {
                    split_info.first
                };
                pane.resize(self.pane_content_size(pane.pane_id(), incoming_size))?;
                // Pre-emptively resize the tab contents down to
                // match the target size; it's easier to reuse
                // existing resize logic that way
                if request.target_is_second {
                    self.resize(split_info.first.clone());
                } else {
                    self.resize(split_info.second.clone());
                }
            }

            let mut cursor = self.pane.take().unwrap().cursor();

            if request.top_level && !cursor.is_leaf() {
                let result = if request.target_is_second {
                    cursor.split_node_and_insert_right(Arc::clone(&pane))
                } else {
                    cursor.split_node_and_insert_left(Arc::clone(&pane))
                };
                cursor = match result {
                    Ok(c) => {
                        cursor = match c.assign_node(Some(split_info)) {
                            Err(c) | Ok(c) => c,
                        };

                        self.pane.replace(cursor.tree());
                        self.size = tab_size;
                        self.resize_reference = None;

                        let pane_index = if request.target_is_second {
                            self.pane.as_ref().unwrap().num_leaves().saturating_sub(1)
                        } else {
                            0
                        };

                        self.active = pane_index;
                        self.recency.tag(pane_index);
                        return Ok(pane_index);
                    }
                    Err(cursor) => cursor,
                };
            }

            match cursor.go_to_nth_leaf(pane_index) {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    anyhow::bail!("invalid pane_index {}; cannot split!", pane_index);
                }
            };

            let existing_pane = Arc::clone(cursor.leaf_mut().unwrap());
            let existing_size = cursor
                .path_to_root()
                .find_map(|(branch, node)| {
                    node.as_ref().map(|node| {
                        if branch == PathBranch::IsRight {
                            node.second
                        } else {
                            node.first
                        }
                    })
                })
                .unwrap_or(tab_size);

            let (pane1, pane2) = if request.target_is_second {
                (Arc::clone(&existing_pane), pane)
            } else {
                (pane, Arc::clone(&existing_pane))
            };

            if let Err(error) = pane1
                .resize(self.pane_content_size(pane1.pane_id(), split_info.first))
                .and_then(|()| {
                    pane2.resize(self.pane_content_size(pane2.pane_id(), split_info.second))
                })
            {
                // Never leave self.pane empty when a pty resize fails. In
                // particular, an SSH disconnect can race a drag-and-drop split.
                self.pane.replace(cursor.tree());
                existing_pane
                    .resize(self.pane_content_size(existing_pane.pane_id(), existing_size))
                    .ok();
                return Err(error);
            }

            *cursor.leaf_mut().unwrap() = pane1;

            match cursor.split_leaf_and_insert_right(pane2) {
                Ok(c) => cursor = c,
                Err(c) => {
                    self.pane.replace(c.tree());
                    anyhow::bail!("invalid pane_index {}; cannot split!", pane_index);
                }
            };

            // cursor now points to the newly created split node;
            // we need to populate its split information
            match cursor.assign_node(Some(split_info)) {
                Err(c) | Ok(c) => self.pane.replace(c.tree()),
            };

            if request.target_is_second {
                self.active = pane_index + 1;
                self.recency.tag(pane_index + 1);
            }
        }

        self.resize_reference = None;
        log::debug!("split info after split: {:#?}", self.iter_splits());
        log::debug!("pane info after split: {:#?}", self.iter_panes());

        Ok(if request.target_is_second {
            pane_index + 1
        } else {
            pane_index
        })
    }

    fn get_zoomed_pane(&self) -> Option<Arc<dyn Pane>> {
        self.zoomed.clone()
    }
}

/// This type is used directly by the codec, take care to bump
/// the codec version if you change this
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub enum PaneNode {
    Empty,
    Split {
        left: Box<PaneNode>,
        right: Box<PaneNode>,
        node: SplitDirectionAndSize,
    },
    Leaf(PaneEntry),
}

impl PaneNode {
    pub fn into_tree(self) -> bintree::Tree<PaneEntry, SplitDirectionAndSize> {
        match self {
            PaneNode::Empty => bintree::Tree::Empty,
            PaneNode::Split { left, right, node } => bintree::Tree::Node {
                left: Box::new((*left).into_tree()),
                right: Box::new((*right).into_tree()),
                data: Some(node),
            },
            PaneNode::Leaf(e) => bintree::Tree::Leaf(e),
        }
    }

    pub fn root_size(&self) -> Option<TerminalSize> {
        match self {
            PaneNode::Empty => None,
            PaneNode::Split { node, .. } => Some(node.size()),
            PaneNode::Leaf(entry) => Some(entry.size),
        }
    }

    pub fn window_and_tab_ids(&self) -> Option<(WindowId, TabId)> {
        match self {
            PaneNode::Empty => None,
            PaneNode::Split { left, right, .. } => match left.window_and_tab_ids() {
                Some(res) => Some(res),
                None => right.window_and_tab_ids(),
            },
            PaneNode::Leaf(entry) => Some((entry.window_id, entry.tab_id)),
        }
    }
}

/// This type is used directly by the codec, take care to bump
/// the codec version if you change this
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct PaneEntry {
    pub window_id: WindowId,
    pub tab_id: TabId,
    pub pane_id: PaneId,
    pub title: String,
    pub size: TerminalSize,
    pub working_dir: Option<SerdeUrl>,
    pub is_active_pane: bool,
    pub is_zoomed_pane: bool,
    pub workspace: String,
    pub cursor_pos: StableCursorPosition,
    pub physical_top: StableRowIndex,
    pub top_row: usize,
    pub left_col: usize,
    pub tty_name: Option<String>,
}

#[derive(Deserialize, Clone, Serialize, PartialEq, Debug)]
#[serde(try_from = "String", into = "String")]
pub struct SerdeUrl {
    pub url: Url,
}

impl std::convert::TryFrom<String> for SerdeUrl {
    type Error = url::ParseError;
    fn try_from(s: String) -> Result<SerdeUrl, url::ParseError> {
        let url = Url::parse(&s)?;
        Ok(SerdeUrl { url })
    }
}

impl From<Url> for SerdeUrl {
    fn from(url: Url) -> SerdeUrl {
        SerdeUrl { url }
    }
}

impl Into<Url> for SerdeUrl {
    fn into(self) -> Url {
        self.url
    }
}

impl Into<String> for SerdeUrl {
    fn into(self) -> String {
        self.url.as_str().into()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::renderable::*;
    use parking_lot::{MappedMutexGuard, Mutex};
    use rangeset::RangeSet;
    use std::ops::Range;
    use termwiz::surface::SequenceNo;
    use url::Url;
    use wezterm_term::color::ColorPalette;
    use wezterm_term::{KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex};

    struct FakePane {
        id: PaneId,
        size: Mutex<TerminalSize>,
        resize_failures: Mutex<usize>,
        resize_count: Mutex<usize>,
    }

    impl FakePane {
        fn new(id: PaneId, size: TerminalSize) -> Arc<dyn Pane> {
            Arc::new(Self {
                id,
                size: Mutex::new(size),
                resize_failures: Mutex::new(0),
                resize_count: Mutex::new(0),
            })
        }
    }

    impl Pane for FakePane {
        fn pane_id(&self) -> PaneId {
            self.id
        }

        fn get_cursor_position(&self) -> StableCursorPosition {
            unimplemented!();
        }

        fn get_current_seqno(&self) -> SequenceNo {
            unimplemented!();
        }

        fn get_changed_since(
            &self,
            _lines: Range<StableRowIndex>,
            _: SequenceNo,
        ) -> RangeSet<StableRowIndex> {
            unimplemented!();
        }

        fn with_lines_mut(
            &self,
            _stable_range: Range<StableRowIndex>,
            _with_lines: &mut dyn WithPaneLines,
        ) {
            unimplemented!();
        }

        fn for_each_logical_line_in_stable_range_mut(
            &self,
            _lines: Range<StableRowIndex>,
            _for_line: &mut dyn ForEachPaneLogicalLine,
        ) {
            unimplemented!();
        }

        fn get_lines(&self, _lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
            unimplemented!();
        }

        fn get_logical_lines(&self, _lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
            unimplemented!();
        }

        fn get_dimensions(&self) -> RenderableDimensions {
            unimplemented!();
        }

        fn get_title(&self) -> String {
            unimplemented!()
        }
        fn send_paste(&self, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
            Ok(None)
        }
        fn writer(&self) -> MappedMutexGuard<'_, dyn std::io::Write> {
            unimplemented!()
        }
        fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
            *self.resize_count.lock() += 1;
            let mut failures = self.resize_failures.lock();
            if *failures > 0 {
                *failures -= 1;
                anyhow::bail!("injected pty resize failure");
            }
            *self.size.lock() = size;
            Ok(())
        }

        fn key_down(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn key_up(&self, _: KeyCode, _: KeyModifiers) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn mouse_event(&self, _event: MouseEvent) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn is_dead(&self) -> bool {
            false
        }
        fn palette(&self) -> ColorPalette {
            unimplemented!()
        }
        fn domain_id(&self) -> DomainId {
            1
        }
        fn is_mouse_grabbed(&self) -> bool {
            false
        }
        fn is_alt_screen_active(&self) -> bool {
            false
        }
        fn get_current_working_dir(&self, _policy: CachePolicy) -> Option<Url> {
            None
        }
    }

    fn pane_layout(tab: &Tab) -> Vec<(PaneId, usize, usize, usize, usize)> {
        tab.iter_panes_ignoring_zoom()
            .iter()
            .map(|p| (p.pane.pane_id(), p.left, p.top, p.width, p.height))
            .collect()
    }

    fn fail_next_resizes(pane: &Arc<dyn Pane>, count: usize) {
        *pane
            .downcast_ref::<FakePane>()
            .unwrap()
            .resize_failures
            .lock() = count;
    }

    fn fake_size(pane: &Arc<dyn Pane>) -> TerminalSize {
        *pane.downcast_ref::<FakePane>().unwrap().size.lock()
    }

    #[test]
    fn move_between_tabs_preserves_both_layouts_on_either_resize_failure() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        for target_is_second in [false, true] {
            for fail_existing in [false, true] {
                let target = Tab::new(&size);
                let existing = FakePane::new(1, size);
                target.assign_pane(&existing);
                target
                    .split_and_insert(0, SplitRequest::default(), FakePane::new(3, size))
                    .unwrap();
                let source = Tab::new(&size);
                let incoming = FakePane::new(2, size);
                source.assign_pane(&incoming);
                let target_before = pane_layout(&target);
                let source_before = pane_layout(&source);
                let existing_size = fake_size(&existing);
                fail_next_resizes(if fail_existing { &existing } else { &incoming }, 1);
                let request = SplitRequest {
                    target_is_second,
                    ..Default::default()
                };

                assert!(target.move_pane_from(&source, 2, 1, request).is_err());
                assert_eq!(pane_layout(&target), target_before);
                assert_eq!(pane_layout(&source), source_before);
                assert_eq!(fake_size(&existing), existing_size);
                assert_eq!(fake_size(&incoming), size);

                // The restored tabs remain usable; retry the same move.
                target.move_pane_from(&source, 2, 1, request).unwrap();
                assert!(pane_layout(&source).is_empty());
                let mut ids: Vec<_> = pane_layout(&target).iter().map(|p| p.0).collect();
                ids.sort_unstable();
                assert_eq!(ids, vec![1, 2, 3]);
            }
        }
    }

    #[test]
    fn move_within_tab_restores_layout_and_focus_on_resize_failure() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        for target_is_second in [false, true] {
            let tab = Tab::new(&size);
            let existing = FakePane::new(1, size);
            let incoming = FakePane::new(2, size);
            tab.assign_pane(&existing);
            tab.split_and_insert(0, SplitRequest::default(), Arc::clone(&incoming))
                .unwrap();
            let before = pane_layout(&tab);
            let active_before = tab.get_active_idx();
            let existing_size = fake_size(&existing);
            let incoming_size = fake_size(&incoming);
            fail_next_resizes(&incoming, 1);

            assert!(tab
                .move_pane_from(
                    &tab,
                    2,
                    1,
                    SplitRequest {
                        direction: SplitDirection::Vertical,
                        target_is_second,
                        ..Default::default()
                    }
                )
                .is_err());
            assert_eq!(pane_layout(&tab), before);
            assert_eq!(tab.get_active_idx(), active_before);
            assert_eq!(fake_size(&existing), existing_size);
            assert_eq!(fake_size(&incoming), incoming_size);
        }
    }

    #[test]
    fn failed_move_keeps_source_when_size_rollback_also_fails() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        let target = Tab::new(&size);
        target.assign_pane(&FakePane::new(1, size));
        let source = Tab::new(&size);
        let incoming = FakePane::new(2, size);
        source.assign_pane(&incoming);
        fail_next_resizes(&incoming, 3);
        assert!(target
            .move_pane_from(&source, 2, 1, SplitRequest::default())
            .is_err());
        assert_eq!(pane_layout(&target), vec![(1, 0, 0, 80, 24)]);
        assert_eq!(pane_layout(&source), vec![(2, 0, 0, 80, 24)]);
    }

    #[test]
    fn pane_headers_reserve_pty_rows_through_layout_changes() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        let tab = Tab::new(&size);
        let first = FakePane::new(1, size);
        let second = FakePane::new(2, size);
        tab.assign_pane(&first);
        assert_eq!(fake_size(&first), size);
        tab.set_pane_header_rows(2);
        assert_eq!(fake_size(&first).rows, 22);
        assert_eq!(fake_size(&first).pixel_height, 550);
        assert_eq!(pane_layout(&tab), vec![(1, 0, 0, 80, 24)]);
        tab.split_and_insert(
            0,
            SplitRequest {
                direction: SplitDirection::Vertical,
                ..Default::default()
            },
            Arc::clone(&second),
        )
        .unwrap();
        assert_eq!(fake_size(&first).rows, 9);
        assert_eq!(fake_size(&second).rows, 10);
        tab.resize_split_by(0, 1);
        assert_eq!(fake_size(&first).rows, 10);
        assert_eq!(fake_size(&second).rows, 9);
        tab.set_zoomed(true);
        assert_eq!(fake_size(&second).rows, 22);
        tab.set_zoomed(false);
        assert_eq!(fake_size(&second).rows, 9);
        tab.remove_pane(1);
        assert_eq!(fake_size(&second).rows, 22);
        tab.set_pane_header_rows(0);
        assert_eq!(fake_size(&second), size);
    }

    #[test]
    fn pane_headers_preserve_sizes_when_a_move_rolls_back() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        for target_is_second in [false, true] {
            let target = Tab::new(&size);
            let source = Tab::new(&size);
            let first = FakePane::new(1, size);
            let second = FakePane::new(2, size);
            target.assign_pane(&first);
            source.assign_pane(&second);
            // Different windows can use different DPI/font/header row counts.
            target.set_pane_header_rows(2);
            source.set_pane_header_rows(3);
            let target_size = fake_size(&first);
            let source_size = fake_size(&second);
            fail_next_resizes(&second, 1);
            assert!(target
                .move_pane_from(
                    &source,
                    2,
                    1,
                    SplitRequest {
                        target_is_second,
                        ..Default::default()
                    }
                )
                .is_err());
            assert_eq!(fake_size(&first), target_size);
            assert_eq!(fake_size(&second), source_size);
            assert_eq!(pane_layout(&target), vec![(1, 0, 0, 80, 24)]);
            assert_eq!(pane_layout(&source), vec![(2, 0, 0, 80, 24)]);
        }
    }

    #[test]
    fn pane_headers_apply_to_top_level_splits_and_tiny_panes() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };
        let tab = Tab::new(&size);
        tab.assign_pane(&FakePane::new(1, size));
        tab.set_pane_header_rows(2);
        tab.split_and_insert(0, SplitRequest::default(), FakePane::new(2, size))
            .unwrap();
        let third = FakePane::new(3, size);
        tab.split_and_insert(
            0,
            SplitRequest {
                top_level: true,
                direction: SplitDirection::Vertical,
                ..Default::default()
            },
            Arc::clone(&third),
        )
        .unwrap();
        assert_eq!(tab.get_size(), size);
        for pane in tab.iter_panes() {
            assert_eq!(fake_size(&pane.pane).rows, pane.height - 2);
        }
        let tiny = TerminalSize {
            rows: 1,
            pixel_height: 25,
            ..size
        };
        assert_eq!(pane_content_size(tiny, 2, (0, 0)), tiny);
        let two = TerminalSize {
            rows: 2,
            pixel_height: 50,
            ..size
        };
        assert_eq!(pane_content_size(two, 2, (0, 0)), tiny);
    }

    #[test]
    fn pane_content_padding_is_symmetric_and_uses_physical_pixels() {
        let size = TerminalSize {
            rows: 40,
            cols: 100,
            pixel_width: 1000,
            pixel_height: 800,
            dpi: 96,
        };
        assert_eq!(
            pane_content_size(size, 2, (10, 10)),
            TerminalSize {
                rows: 37,
                cols: 98,
                pixel_width: 980,
                pixel_height: 740,
                dpi: 96,
            }
        );
        assert_eq!(
            pane_size_with_cell(size, 2, (10, 10), Some((20, 25))),
            TerminalSize {
                rows: 29,
                cols: 49,
                pixel_width: 980,
                pixel_height: 725,
                dpi: 96,
            }
        );
    }

    fn proportional_test_size(cols: usize, rows: usize) -> TerminalSize {
        TerminalSize {
            cols,
            rows,
            pixel_width: cols * 10,
            pixel_height: rows * 20,
            dpi: 96,
        }
    }

    fn proportional_test_grid() -> Tab {
        let size = proportional_test_size(101, 81);
        let tab = Tab::new(&size);
        tab.set_proportional_resize(true);
        tab.assign_pane(&FakePane::new(1, size));
        tab.split_and_insert(0, SplitRequest::default(), FakePane::new(2, size))
            .unwrap();
        for (index, id) in [(0, 3), (2, 4)] {
            tab.split_and_insert(
                index,
                SplitRequest {
                    direction: SplitDirection::Vertical,
                    ..Default::default()
                },
                FakePane::new(id, size),
            )
            .unwrap();
        }
        tab
    }

    #[test]
    fn proportional_resize_tracks_every_pane_without_small_step_drift() {
        let tab = proportional_test_grid();
        let original = pane_layout(&tab);
        // Simulate the many one-cell resize events from dragging a window edge.
        for step in 1..=100 {
            tab.resize(proportional_test_size(101 + step, 81 + step));
            let panes = tab.iter_panes();
            for pane in panes {
                assert!((pane.width as isize - (100 + step) as isize / 2).abs() <= 1);
                assert!((pane.height as isize - (80 + step) as isize / 2).abs() <= 1);
                assert_eq!(fake_size(&pane.pane).cols, pane.width);
                assert_eq!(fake_size(&pane.pane).rows, pane.height);
            }
        }
        for step in (0..100).rev() {
            tab.resize(proportional_test_size(101 + step, 81 + step));
        }
        assert_eq!(pane_layout(&tab), original);
    }

    #[test]
    fn proportional_resize_preserves_dragged_ratios_and_child_ratios() {
        let tab = proportional_test_grid();
        // Root horizontal divider 50/50 -> 70/30; left vertical 40/40 -> 50/30.
        tab.resize_split_by(0, 20);
        tab.resize_split_by(1, 10);
        let original = pane_layout(&tab);
        assert_eq!(
            original,
            vec![
                (1, 0, 0, 70, 50),
                (3, 0, 51, 70, 30),
                (2, 71, 0, 30, 40),
                (4, 71, 41, 30, 40)
            ]
        );
        for step in 1..=100 {
            tab.resize(proportional_test_size(101 + step, 81 + step));
        }
        let panes = tab.iter_panes();
        assert_eq!(panes[0].width, 140);
        assert_eq!(panes[1].width, 140);
        assert_eq!(panes[2].width, 60);
        assert_eq!(panes[3].width, 60);
        assert_eq!((panes[0].height, panes[1].height), (113, 67));
        assert_eq!((panes[2].height, panes[3].height), (90, 90));
        tab.resize(proportional_test_size(101, 81));
        assert_eq!(pane_layout(&tab), original);

        // Moving a parent divider must not pin the first nested child either.
        let size = proportional_test_size(101, 81);
        tab.split_and_insert(0, SplitRequest::default(), FakePane::new(5, size))
            .unwrap();
        tab.resize_split_by(0, -30);
        let panes = tab.iter_panes();
        assert!((panes[0].width as isize - panes[1].width as isize).abs() <= 1);
        assert_eq!(panes[0].width + panes[1].width + 1, 40);
    }

    #[test]
    fn proportional_resize_recovers_ratios_after_nested_minimum_clamping() {
        let size = proportional_test_size(101, 81);
        let tab = proportional_test_grid();
        tab.split_and_insert(0, SplitRequest::default(), FakePane::new(5, size))
            .unwrap();
        tab.resize_split_by(0, 20);
        let original = pane_layout(&tab);
        tab.resize(proportional_test_size(1, 1));
        assert_eq!((tab.get_size().cols, tab.get_size().rows), (5, 3));
        for pane in tab.iter_panes() {
            assert!(pane.width >= 1 && pane.height >= 1);
            assert!(pane.left + pane.width <= tab.get_size().cols);
            assert!(pane.top + pane.height <= tab.get_size().rows);
        }
        tab.resize(size);
        assert_eq!(pane_layout(&tab), original);
        // A drag to an extreme cannot shrink a subtree below all its dividers.
        tab.resize_split_by(0, -1000);
        let panes = tab.iter_panes();
        assert_eq!(panes[0].width, 1);
        assert_eq!(panes[1].width, 1);
        assert_eq!(panes[2].width, 3);
        assert!(panes.iter().all(|p| p.width >= 1 && p.height >= 1));
    }

    #[test]
    fn proportional_resize_survives_zoom_and_updates_dpi_without_cell_changes() {
        let tab = proportional_test_grid();
        tab.set_pane_header_rows(2);
        let original = pane_layout(&tab);
        tab.set_zoomed(true);
        tab.resize(proportional_test_size(201, 161));
        assert_eq!(tab.iter_panes().len(), 1);
        tab.set_zoomed(false);
        for pane in tab.iter_panes() {
            assert_eq!(pane.width, 100);
            assert_eq!(pane.height, 80);
            assert_eq!(fake_size(&pane.pane).rows, 78);
        }
        tab.resize(proportional_test_size(101, 81));
        tab.resize(TerminalSize {
            pixel_width: 1515,
            pixel_height: 2430,
            dpi: 144,
            ..proportional_test_size(101, 81)
        });
        assert_eq!(pane_layout(&tab), original);
        for pane in tab.iter_panes() {
            let size = fake_size(&pane.pane);
            assert_eq!(size.dpi, 144);
            assert_eq!(size.pixel_width, pane.width * 15);
            assert_eq!(size.pixel_height, (pane.height - 2) * 30);
        }
    }

    #[test]
    fn proportional_resize_rebases_after_topology_change() {
        let tab = proportional_test_grid();
        tab.resize(proportional_test_size(201, 161));
        tab.remove_pane(4).unwrap();
        tab.split_and_insert(
            2,
            SplitRequest {
                direction: SplitDirection::Vertical,
                size: SplitSize::Percent(25),
                ..Default::default()
            },
            FakePane::new(4, tab.get_size()),
        )
        .unwrap();
        let original = pane_layout(&tab);
        // Same IDs/tree as before, but the newly-created right split is 75/25.
        tab.resize(proportional_test_size(401, 321));
        let panes = tab.iter_panes();
        assert_eq!((panes[2].height, panes[3].height), (240, 80));
        tab.resize(proportional_test_size(201, 161));
        assert_eq!(pane_layout(&tab), original);
    }

    #[test]
    fn proportional_resize_avoids_repeated_pty_resizes_but_applies_header_changes() {
        let size = proportional_test_size(101, 81);
        let tab = proportional_test_grid();
        let panes = tab.iter_panes();
        let resize_count = || {
            panes
                .iter()
                .map(|pane| {
                    *pane
                        .pane
                        .downcast_ref::<FakePane>()
                        .unwrap()
                        .resize_count
                        .lock()
                })
                .collect::<Vec<_>>()
        };
        let before = resize_count();
        for _ in 0..20 {
            tab.resize(size);
        }
        assert_eq!(resize_count(), before);
        tab.set_pane_header_rows(2);
        assert_eq!(
            resize_count(),
            before.iter().map(|count| count + 1).collect::<Vec<_>>()
        );
        for pane in tab.iter_panes() {
            assert_eq!(fake_size(&pane.pane).rows, pane.height - 2);
        }
    }

    #[test]
    fn pane_font_override_changes_only_its_pty_grid_and_survives_window_resize() {
        let tab = proportional_test_grid();
        tab.set_pane_header_rows(2);
        let original = pane_layout(&tab);
        let panes = tab.iter_panes();
        let pane = &panes[0].pane;
        let untouched = fake_size(&panes[1].pane);
        tab.set_pane_cell_size(1, Some((20, 30)));
        assert_eq!(pane_layout(&tab), original);
        assert_eq!(fake_size(&panes[1].pane), untouched);
        assert_eq!(
            fake_size(pane),
            TerminalSize {
                cols: 25,
                rows: 25,
                pixel_width: 500,
                pixel_height: 750,
                dpi: 96,
            }
        );
        let count = *pane.downcast_ref::<FakePane>().unwrap().resize_count.lock();
        for _ in 0..10 {
            tab.set_pane_cell_size(1, Some((20, 30)));
        }
        assert_eq!(
            *pane.downcast_ref::<FakePane>().unwrap().resize_count.lock(),
            count
        );
        tab.resize(proportional_test_size(201, 161));
        assert_eq!(
            fake_size(pane),
            TerminalSize {
                cols: 50,
                rows: 52,
                pixel_width: 1000,
                pixel_height: 1560,
                dpi: 96,
            }
        );
        assert_eq!(
            *pane.downcast_ref::<FakePane>().unwrap().resize_count.lock(),
            count + 1
        );
        tab.set_pane_cell_size(1, None);
        assert_eq!((fake_size(pane).cols, fake_size(pane).rows), (100, 78));
    }

    #[test]
    fn pane_font_override_handles_zoom_tiny_panes_and_removal() {
        let tab = proportional_test_grid();
        tab.set_pane_header_rows(2);
        let pane = tab.get_active_pane().unwrap();
        let id = pane.pane_id();
        tab.set_pane_cell_size(id, Some((20, 30)));
        tab.set_zoomed(true);
        assert_eq!((fake_size(&pane).cols, fake_size(&pane).rows), (50, 52));
        tab.set_zoomed(false);
        assert_eq!((fake_size(&pane).cols, fake_size(&pane).rows), (25, 25));
        tab.resize(proportional_test_size(1, 1));
        assert_eq!((fake_size(&pane).cols, fake_size(&pane).rows), (1, 1));
        tab.remove_pane(id).unwrap();
        assert!(!tab.inner.lock().pane_cell_sizes.contains_key(&id));
    }

    #[test]
    fn pane_font_override_transfers_between_tabs_and_survives_failed_move() {
        let size = proportional_test_size(101, 81);
        let source = Tab::new(&size);
        let target = Tab::new(&size);
        let incoming = FakePane::new(1, size);
        source.assign_pane(&incoming);
        target.assign_pane(&FakePane::new(2, size));
        source.set_pane_header_rows(2);
        target.set_pane_header_rows(3);
        source.set_pane_cell_size(1, Some((20, 30)));
        let original_size = fake_size(&incoming);
        fail_next_resizes(&incoming, 1);
        assert!(target
            .move_pane_from(&source, 1, 2, SplitRequest::default())
            .is_err());
        assert_eq!(fake_size(&incoming), original_size);
        assert_eq!(source.inner.lock().pane_cell_sizes.get(&1), Some(&(20, 30)));
        assert!(!target.inner.lock().pane_cell_sizes.contains_key(&1));
        target
            .move_pane_from(&source, 1, 2, SplitRequest::default())
            .unwrap();
        assert!(!source.inner.lock().pane_cell_sizes.contains_key(&1));
        assert_eq!(target.inner.lock().pane_cell_sizes.get(&1), Some(&(20, 30)));
        assert_eq!(
            fake_size(&incoming),
            TerminalSize {
                cols: 25,
                rows: 52,
                pixel_width: 500,
                pixel_height: 1560,
                dpi: 96,
            }
        );
    }

    #[test]
    fn pane_font_override_survives_same_tab_rearrangement_and_rollback() {
        let tab = proportional_test_grid();
        let pane = tab
            .iter_panes()
            .into_iter()
            .find(|p| p.pane.pane_id() == 1)
            .unwrap()
            .pane;
        tab.set_pane_cell_size(1, Some((20, 30)));
        let original = pane_layout(&tab);
        let original_size = fake_size(&pane);
        fail_next_resizes(&pane, 1);
        assert!(tab
            .move_pane_from(&tab, 1, 2, SplitRequest::default())
            .is_err());
        assert_eq!(pane_layout(&tab), original);
        assert_eq!(fake_size(&pane), original_size);
        assert_eq!(tab.inner.lock().pane_cell_sizes.get(&1), Some(&(20, 30)));
        tab.move_pane_from(&tab, 1, 2, SplitRequest::default())
            .unwrap();
        assert_eq!(tab.inner.lock().pane_cell_sizes.get(&1), Some(&(20, 30)));
        let position = tab
            .iter_panes()
            .into_iter()
            .find(|p| p.pane.pane_id() == 1)
            .unwrap();
        assert_eq!(fake_size(&pane).cols, (position.pixel_width / 20).max(1));
        assert_eq!(fake_size(&pane).rows, (position.pixel_height / 30).max(1));
    }

    #[test]
    fn proportional_resize_is_opt_in_for_native_gui_tabs() {
        let size = proportional_test_size(101, 81);
        let tab = Tab::new(&size);
        tab.assign_pane(&FakePane::new(1, size));
        tab.split_and_insert(0, SplitRequest::default(), FakePane::new(2, size))
            .unwrap();
        // Preserve existing mux behavior, including its first-child bias.
        for step in 1..=10 {
            tab.resize(proportional_test_size(101 + step, 81));
        }
        let panes = tab.iter_panes();
        assert_eq!((panes[0].width, panes[1].width), (60, 50));
    }

    #[test]
    fn tab_splitting() {
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
            dpi: 96,
        };

        let tab = Tab::new(&size);
        tab.assign_pane(&FakePane::new(1, size));

        let panes = tab.iter_panes();
        assert_eq!(1, panes.len());
        assert_eq!(0, panes[0].index);
        assert_eq!(true, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(80, panes[0].width);
        assert_eq!(24, panes[0].height);

        assert!(tab
            .compute_split_size(
                1,
                SplitRequest {
                    direction: SplitDirection::Horizontal,
                    ..Default::default()
                }
            )
            .is_none());

        let horz_size = tab
            .compute_split_size(
                0,
                SplitRequest {
                    direction: SplitDirection::Horizontal,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            horz_size,
            SplitDirectionAndSize {
                direction: SplitDirection::Horizontal,
                second: TerminalSize {
                    rows: 24,
                    cols: 40,
                    pixel_width: 400,
                    pixel_height: 600,
                    dpi: 96,
                },
                first: TerminalSize {
                    rows: 24,
                    cols: 39,
                    pixel_width: 390,
                    pixel_height: 600,
                    dpi: 96,
                },
            }
        );

        let vert_size = tab
            .compute_split_size(
                0,
                SplitRequest {
                    direction: SplitDirection::Vertical,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            vert_size,
            SplitDirectionAndSize {
                direction: SplitDirection::Vertical,
                second: TerminalSize {
                    rows: 12,
                    cols: 80,
                    pixel_width: 800,
                    pixel_height: 300,
                    dpi: 96,
                },
                first: TerminalSize {
                    rows: 11,
                    cols: 80,
                    pixel_width: 800,
                    pixel_height: 275,
                    dpi: 96,
                }
            }
        );

        let new_index = tab
            .split_and_insert(
                0,
                SplitRequest {
                    direction: SplitDirection::Horizontal,
                    ..Default::default()
                },
                FakePane::new(2, horz_size.second),
            )
            .unwrap();
        assert_eq!(new_index, 1);

        let panes = tab.iter_panes();
        assert_eq!(2, panes.len());

        assert_eq!(0, panes[0].index);
        assert_eq!(false, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(39, panes[0].width);
        assert_eq!(24, panes[0].height);
        assert_eq!(390, panes[0].pixel_width);
        assert_eq!(600, panes[0].pixel_height);
        assert_eq!(1, panes[0].pane.pane_id());

        assert_eq!(1, panes[1].index);
        assert_eq!(true, panes[1].is_active);
        assert_eq!(40, panes[1].left);
        assert_eq!(0, panes[1].top);
        assert_eq!(40, panes[1].width);
        assert_eq!(24, panes[1].height);
        assert_eq!(400, panes[1].pixel_width);
        assert_eq!(600, panes[1].pixel_height);
        assert_eq!(2, panes[1].pane.pane_id());

        let vert_size = tab
            .compute_split_size(
                0,
                SplitRequest {
                    direction: SplitDirection::Vertical,
                    ..Default::default()
                },
            )
            .unwrap();
        let new_index = tab
            .split_and_insert(
                0,
                SplitRequest {
                    direction: SplitDirection::Vertical,
                    top_level: false,
                    target_is_second: true,
                    size: Default::default(),
                },
                FakePane::new(3, vert_size.second),
            )
            .unwrap();
        assert_eq!(new_index, 1);

        let panes = tab.iter_panes();
        assert_eq!(3, panes.len());

        assert_eq!(0, panes[0].index);
        assert_eq!(false, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(39, panes[0].width);
        assert_eq!(11, panes[0].height);
        assert_eq!(390, panes[0].pixel_width);
        assert_eq!(275, panes[0].pixel_height);
        assert_eq!(1, panes[0].pane.pane_id());

        assert_eq!(1, panes[1].index);
        assert_eq!(true, panes[1].is_active);
        assert_eq!(0, panes[1].left);
        assert_eq!(12, panes[1].top);
        assert_eq!(39, panes[1].width);
        assert_eq!(12, panes[1].height);
        assert_eq!(390, panes[1].pixel_width);
        assert_eq!(300, panes[1].pixel_height);
        assert_eq!(3, panes[1].pane.pane_id());

        assert_eq!(2, panes[2].index);
        assert_eq!(false, panes[2].is_active);
        assert_eq!(40, panes[2].left);
        assert_eq!(0, panes[2].top);
        assert_eq!(40, panes[2].width);
        assert_eq!(24, panes[2].height);
        assert_eq!(400, panes[2].pixel_width);
        assert_eq!(600, panes[2].pixel_height);
        assert_eq!(2, panes[2].pane.pane_id());

        tab.resize_split_by(1, 1);
        let panes = tab.iter_panes();
        assert_eq!(39, panes[0].width);
        assert_eq!(12, panes[0].height);
        assert_eq!(390, panes[0].pixel_width);
        assert_eq!(300, panes[0].pixel_height);

        assert_eq!(39, panes[1].width);
        assert_eq!(11, panes[1].height);
        assert_eq!(390, panes[1].pixel_width);
        assert_eq!(275, panes[1].pixel_height);

        assert_eq!(40, panes[2].width);
        assert_eq!(24, panes[2].height);
        assert_eq!(400, panes[2].pixel_width);
        assert_eq!(600, panes[2].pixel_height);
    }

    fn is_send_and_sync<T: Send + Sync>() -> bool {
        true
    }

    #[test]
    fn tab_is_send_and_sync() {
        assert!(is_send_and_sync::<Tab>());
    }
}
