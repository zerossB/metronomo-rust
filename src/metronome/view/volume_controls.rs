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
            .flex_col()
            .gap_2()
            .p_2()
            .border_1()
            .border_color(cx.theme().transparent)
            .rounded(cx.theme().radius)
            .focus_visible(|style| style.border_color(cx.theme().ring))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_sm()
                    .child("Volume")
                    .child(
                        div()
                            .id("volume-value")
                            .test_support()
                            .aria_label(format!("{volume:.0}%"))
                            .text_color(cx.theme().muted_foreground)
                            .child(if volume == 0. {
                                "Muted".into()
                            } else {
                                format!("{volume:.0}%")
                            }),
                    ),
            )
            .child(
                Slider::new(&self.volume)
                    .disabled(self.closing)
                    .accessibility_label("Volume percentage"),
            )
    }
}
