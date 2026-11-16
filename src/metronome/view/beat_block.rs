//! A value-like beat presentation that exposes a native Button for menu composition.

use crate::metronome::{audio::PlaybackPosition, meter::BeatSound};
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
};
use gpui_kit::{
    App, InteractiveElement as _, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _, div,
};

pub(super) struct BeatBlock {
    beat: u8,
    sound: BeatSound,
    position: PlaybackPosition,
    menu_open: bool,
}

impl BeatBlock {
    pub(super) fn new(
        beat: u8,
        sound: BeatSound,
        position: PlaybackPosition,
        menu_open: bool,
    ) -> Self {
        Self {
            beat,
            sound,
            position,
            menu_open,
        }
    }

    /// The caller retains focus and attaches the framework's keyboard-aware menu.
    pub(super) fn into_button(self, cx: &App) -> Button {
        let number = u32::from(self.beat + 1);
        let playing = self.position.beat() == Some(self.beat);
        let label = format!(
            "Beat {number}: {}{}{}",
            self.sound.label(),
            if self.beat == 0 { ", accented" } else { "" },
            if playing { ", playing" } else { "" }
        );
        let theme = cx.theme();
        let background = if playing {
            theme
                .primary
                .opacity(if self.position.is_pulse() { 0.22 } else { 0.10 })
        } else {
            theme.background
        };
        Button::new(("beat-sound", number))
            .accessibility_label(label)
            .tooltip(format!(
                "Beat {number}: {}. Choose a sound.",
                self.sound.label()
            ))
            .outline()
            // A beat is a stacked display/control, with room for number, sound,
            // and retained playhead text. Geometry scales with the base font.
            .h_20()
            .w_full()
            .relative()
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(background)
                    .foreground(theme.foreground)
                    .hover(if playing {
                        theme
                            .primary
                            .opacity(if self.position.is_pulse() { 0.28 } else { 0.18 })
                    } else {
                        theme.muted
                    })
                    .active(if playing {
                        theme
                            .primary
                            .opacity(if self.position.is_pulse() { 0.30 } else { 0.22 })
                    } else {
                        theme.muted
                    }),
            )
            .border_color(if self.menu_open {
                theme.foreground
            } else if playing {
                theme.primary
            } else {
                theme.border
            })
            .child(
                div()
                    .absolute()
                    .top_2()
                    .left_2()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if self.beat == 0 { "★" } else { "" }),
            )
            .child(
                Icon::new(if self.menu_open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .absolute()
                .top_2()
                .right_2()
                .size_3()
                .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .w_full()
                    .child(
                        div()
                            .text_size(gpui_kit::rems(2.))
                            .line_height(gpui_kit::relative(1.))
                            .child(number.to_string()),
                    )
                    .child(div().text_xs().child(self.sound.label()))
                    .child(
                        div()
                            .id(("beat-playing", number))
                            .test_support()
                            .aria_label(if playing { "Playing" } else { "Inactive" })
                            .h_4()
                            .text_xs()
                            .child(if playing { "Playing" } else { "" }),
                    ),
            )
    }
}
