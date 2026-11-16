//! Presentation of the volume controls region.

use super::{DecreaseVolume, IncreaseVolume, Metronome};
use gpui_kit::MouseButton;
use gpui_kit::component::{ActiveTheme as _, slider::Slider};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
};

impl Metronome {
    pub(super) fn render_volume(&self, cx: &Context<Self>) -> impl IntoElement {
        let volume = self.volume(cx);
        div()
            .id("volume-controls")
            .test_support()
            .track_focus(&self.volume_focus)
            .key_context("VolumeControls")
            .role(Role::Group)
            .aria_label("Volume. Use arrow keys to adjust.")
            .capture_any_mouse_down(cx.listener(
                |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                    if event.button == MouseButton::Left {
                        window.focus(&this.volume_focus, cx);
                    }
                },
            ))
            .on_action(cx.listener(|this, _: &IncreaseVolume, window, cx| {
                this.adjust_volume(5., window, cx)
            }))
            .on_action(cx.listener(|this, _: &DecreaseVolume, window, cx| {
                this.adjust_volume(-5., window, cx)
            }))
            .flex()
            .items_center()
            .gap_2()
            .px_0()
            .py_1()
            .border_1()
            .border_color(cx.theme().transparent)
            .rounded(cx.theme().radius)
            .focus_visible(|style| style.border_color(cx.theme().ring))
            .child(div().text_sm().flex_shrink_0().child("Volume"))
            .child(
                div().flex_1().min_w_0().px_2().child(
                    Slider::new(&self.volume)
                        .disabled(self.closing)
                        .accessibility_label("Volume percentage"),
                ),
            )
            .child(
                div()
                    .id("volume-value")
                    .test_support()
                    .aria_label(format!("{volume:.0}%"))
                    .w_12()
                    .text_right()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if volume == 0. {
                        "Muted".into()
                    } else {
                        format!("{volume:.0}%")
                    }),
            )
    }
}
