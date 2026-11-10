//! Shared empty and error states. All surfaces use the active TermViAI palette.
use super::*;
impl AppView {
    pub(super) fn interval_button(&self) -> Stateful<Div> {
        div()
            .id("monitor-interval")
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(32.))
            .rounded_md()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .cursor_pointer()
            .hover(|d| d.bg(self.c(self.col().hover)))
            .child(
                svg()
                    .path("termviai/clock.svg")
                    .size(px(14.))
                    .text_color(self.c(self.col().muted)),
            )
            .child(
                self.muted(format!("Every {}s", self.monitor.interval))
                    .text_color(self.c(self.col().text)),
            )
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(px(14.))
                    .text_color(self.c(self.col().muted)),
            )
    }

    pub(super) fn state_icon(&self, icon: IconName, error: bool) -> Div {
        let path = match icon {
            IconName::SquareTerminal => Some("termviai/server.svg"),
            IconName::File => Some("termviai/key.svg"),
            IconName::GalleryVerticalEnd => Some("termviai/layers.svg"),
            IconName::Globe => Some("termviai/waypoints.svg"),
            IconName::Replace => Some("termviai/link-off.svg"),
            IconName::ExternalLink => Some("termviai/wifi-off.svg"),
            _ => None,
        };
        let color = self.c(if error {
            self.col().red
        } else {
            self.col().accent
        });
        let glyph = if let Some(path) = path {
            svg()
                .path(path)
                .size(px(21.))
                .text_color(color)
                .into_any_element()
        } else {
            Icon::new(icon)
                .size(px(21.))
                .text_color(color)
                .into_any_element()
        };
        div()
            .size(px(40.))
            .flex_shrink_0()
            .rounded_lg()
            .bg(self.c(self.col().soft))
            .flex()
            .items_center()
            .justify_center()
            .child(glyph)
    }
    pub(super) fn empty_state(
        &self,
        icon: IconName,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Div {
        div()
            .w_full()
            .flex_shrink_0()
            .py_8()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(self.state_icon(icon, false))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_center()
                    .child(title.into()),
            )
            .child(
                self.muted(description)
                    .text_size(px(13.))
                    .max_w(px(360.))
                    .text_center(),
            )
    }
    pub(super) fn compact_state(&self, icon: IconName, title: &str, description: &str) -> Div {
        div()
            .w_full()
            .flex_shrink_0()
            .py_4()
            .border_b_1()
            .border_color(self.c(self.col().line))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(self.state_icon(icon, false).size(px(34.)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(120.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(title.to_owned()),
                    )
                    .child(self.muted(description.to_owned())),
            )
    }
    pub(super) fn notice(&self, title: &str, description: impl Into<SharedString>) -> Div {
        div()
            .w_full()
            .flex_shrink_0()
            .p_3()
            .rounded_lg()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .flex()
            .items_start()
            .gap_3()
            .child(
                Icon::new(IconName::TriangleAlert)
                    .size(px(16.))
                    .text_color(self.c(self.col().red)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(title.to_owned()),
                    )
                    .child(self.muted(description)),
            )
    }
    pub(super) fn action_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: IconName,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .px_3()
            .h(px(32.))
            .flex_shrink_0()
            .rounded_md()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .cursor_pointer()
            .child(Icon::new(icon).size(px(15.)))
            .child(label.into())
            .hover(|d| {
                d.bg(self.c(self.col().soft))
                    .text_color(self.c(self.col().accent))
            })
    }
}
