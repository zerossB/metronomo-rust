//! Presentation of the tempo controls region.

use super::{MAX_BPM, MIN_BPM, Metronome};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, button::Button, slider::Slider,
};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
};

impl Metronome {
    pub(super) fn render_tempo_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let bpm = self.bpm(cx);
        let tap_hint = match self.session.read(cx).taps() {
            0 => "Tap quarter notes · T",
            1 => "Tap again · T",
            _ => "Keep tapping · T",
        };
        let decrease = Button::new("decrease-tempo")
            .track_focus(&self.tempo_focus[0])
            .icon(IconName::Minus)
            .accessibility_label("Decrease tempo")
            .tooltip("Decrease tempo (↓)")
            .outline()
            .large()
            .disabled(self.closing || bpm <= MIN_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(-1., window, cx);
            }));
        let increase = Button::new("increase-tempo")
            .track_focus(&self.tempo_focus[1])
            .icon(IconName::Plus)
            .accessibility_label("Increase tempo")
            .tooltip("Increase tempo (↑)")
            .outline()
            .large()
            .disabled(self.closing || bpm >= MAX_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(1., window, cx);
            }));

        div()
            .key_context("TempoControls")
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_4()
                            .child(decrease)
                            .child(
                                div()
                                    .id("tempo-value")
                                    .test_support()
                                    .aria_label(format!("{bpm:.0} BPM"))
                                    .w_32()
                                    .text_center()
                                    .text_size(gpui_kit::rems(3.))
                                    .line_height(gpui_kit::relative(1.))
                                    .child(format!("{bpm:.0}")),
                            )
                            .child(increase),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("BPM · 1/4 note"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Slider::new(&self.tempo)
                            .disabled(self.closing)
                            .accessibility_label("Tempo in quarter-note BPM"),
                    )
                    .child(
                        div()
                            .id("tempo-range")
                            .test_support()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{MIN_BPM:.0}"))
                            .child(format!("{MAX_BPM:.0}")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("tap-tempo")
                            .disabled(self.closing)
                            .track_focus(&self.tempo_focus[2])
                            .label("Tap tempo")
                            .tooltip("Tap quarter notes to set the tempo (T)")
                            .outline()
                            .large()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.tap_tempo(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("tap-status")
                            .test_support()
                            .role(Role::Status)
                            .aria_label(tap_hint)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tap_hint),
                    ),
            )
    }
}
