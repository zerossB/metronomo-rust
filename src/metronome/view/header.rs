//! Presentation of the header region.

use super::{Metronome, PlaybackState};
use gpui_kit::component::{ActiveTheme as _, TitleBar};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
};

impl Metronome {
    pub(super) fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let status = if self.audio_error(cx).is_some() {
            "Unavailable"
        } else {
            match self.playback(cx) {
                PlaybackState::Stopped => "Stopped",
                PlaybackState::Starting => "Starting",
                PlaybackState::Running => "Running",
            }
        };

        TitleBar::new()
            .on_close_window(cx.listener(|this, _, window, cx| this.request_close(window, cx)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .mr_3()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("Metronomo"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("beat-indicator")
                                    .test_support()
                                    .aria_label(if self.playback(cx) == PlaybackState::Stopped {
                                        "Stopped"
                                    } else if self.position(cx).is_pulse() {
                                        "Beat"
                                    } else {
                                        "Between beats"
                                    })
                                    .size_3()
                                    .rounded(cx.theme().radius_full())
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(if self.position(cx).is_pulse() {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().muted
                                    }),
                            )
                            .child(
                                div()
                                    .id("playback-status")
                                    .role(Role::Status)
                                    .test_support()
                                    .aria_label(status)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(status),
                            ),
                    ),
            )
    }
}
