//! Playback command and output feedback.

use super::{Metronome, PlaybackState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
};

impl Metronome {
    pub(super) fn render_transport(&self, cx: &Context<Self>) -> impl IntoElement {
        let running = self.playback(cx) != PlaybackState::Stopped;
        let toggle_label = if running { "Stop" } else { "Start" };
        let message = self
            .audio_error(cx)
            .unwrap_or("Default audio output")
            .to_owned();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                Button::new("toggle-playback")
                    .disabled(self.closing)
                    .label(toggle_label)
                    .icon(if running {
                        IconName::Square
                    } else {
                        IconName::Play
                    })
                    .primary()
                    .large()
                    .w_full()
                    .tooltip("Start or stop (Space)")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx))),
            )
            .children(self.settings_error(cx).map(|message| {
                div()
                    .id("settings-message")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(message)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(message)
            }))
            .child(
                div()
                    .text_center()
                    .text_xs()
                    .id("audio-message")
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(if self.audio_error(cx).is_some() {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(message),
            )
    }
}
