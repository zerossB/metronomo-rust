//! Meter controls and sound menus share the feature view's state and audio playhead.

use super::{Metronome, beat_block::BeatBlock};
use crate::metronome::meter::{BeatSound, BeatUnit};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, button::Button};
use gpui_kit::{App, FocusHandle, Subscription, Window};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
};

pub(super) struct MeterFocus {
    count: FocusHandle,
    unit: FocusHandle,
    count_anchor: gpui_kit::ScrollAnchor,
    unit_anchor: gpui_kit::ScrollAnchor,
    beat_anchors: [gpui_kit::ScrollAnchor; super::super::meter::MAX_BEATS as usize],
    beats: [FocusHandle; super::super::meter::MAX_BEATS as usize],
}

impl MeterFocus {
    pub(super) fn new(cx: &App, scroll: &gpui_kit::ScrollHandle) -> Self {
        Self {
            count_anchor: gpui_kit::ScrollAnchor::for_handle(scroll.clone()),
            unit_anchor: gpui_kit::ScrollAnchor::for_handle(scroll.clone()),
            beat_anchors: std::array::from_fn(|_| {
                gpui_kit::ScrollAnchor::for_handle(scroll.clone())
            }),
            count: cx.focus_handle().tab_stop(true),
            unit: cx.focus_handle().tab_stop(true),
            beats: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
        }
    }
    pub(super) fn observe(
        &self,
        window: &mut Window,
        cx: &mut Context<Metronome>,
    ) -> Vec<Subscription> {
        let controls = std::iter::once((self.count.clone(), self.count_anchor.clone()))
            .chain(std::iter::once((
                self.unit.clone(),
                self.unit_anchor.clone(),
            )))
            .chain(
                self.beats
                    .iter()
                    .cloned()
                    .zip(self.beat_anchors.iter().cloned()),
            );
        controls
            .map(|(focus, anchor)| {
                cx.on_focus(&focus, window, move |_, window, cx| {
                    if window.last_input_was_keyboard() {
                        anchor.scroll_to(window, cx);
                        cx.notify();
                    }
                })
            })
            .collect()
    }
}

impl Metronome {
    pub(super) fn render_meter(&self, cx: &Context<Self>) -> impl IntoElement {
        let meter = self.meter(cx);
        let owner = cx.entity().downgrade();
        let count_owner = owner.clone();
        let unit_owner = owner.clone();
        let current = self.position(cx).beat();
        let status = current.map_or_else(
            || format!("{} / {}", meter.beats(), meter.unit().denominator()),
            |beat| format!("Beat {} of {}", beat + 1, meter.beats()),
        );
        let columns = if meter.beats() <= 4 {
            u16::from(meter.beats())
        } else {
            4
        };

        div()
            .id("meter-controls")
            .key_context("MeterControls")
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(anchored(
                        "count-scroll-anchor",
                        self.meter_focus.count_anchor.clone(),
                        Button::new("beats-per-bar")
                            .track_focus(&self.meter_focus.count)
                            .label(format!("{} beats", meter.beats()))
                            .disabled(self.closing)
                            .dropdown_caret(true)
                            .accessibility_label("Beats per bar")
                            .outline()
                            .w_full()
                            .dropdown_menu(move |menu, _, _| {
                                (1..=super::super::meter::MAX_BEATS).fold(menu, |menu, beats| {
                                    let owner = count_owner.clone();
                                    menu.item(
                                        PopupMenuItem::new(format!("{beats} beats"))
                                            .checked(beats == meter.beats())
                                            .on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.set_meter(
                                                        this.meter(cx).with_beats(beats),
                                                        cx,
                                                    )
                                                });
                                            }),
                                    )
                                })
                            })
                            .on_open_change(cx.listener(|_, open, window, cx| {
                                if !open {
                                    cx.defer_in(window, |this, window, cx| {
                                        window.focus(&this.meter_focus.count, cx)
                                    });
                                }
                            })),
                    ))
                    .child(anchored(
                        "unit-scroll-anchor",
                        self.meter_focus.unit_anchor.clone(),
                        Button::new("beat-unit")
                            .track_focus(&self.meter_focus.unit)
                            .label(format!("1/{} note", meter.unit().denominator()))
                            .disabled(self.closing)
                            .dropdown_caret(true)
                            .accessibility_label("Beat unit")
                            .outline()
                            .w_full()
                            .dropdown_menu(move |menu, _, _| {
                                BeatUnit::ALL.into_iter().fold(menu, |menu, unit| {
                                    let owner = unit_owner.clone();
                                    menu.item(
                                        PopupMenuItem::new(format!(
                                            "1/{} · {}",
                                            unit.denominator(),
                                            unit.label()
                                        ))
                                        .checked(unit == meter.unit())
                                        .on_click(
                                            move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.set_meter(
                                                        this.meter(cx).with_unit(unit),
                                                        cx,
                                                    )
                                                });
                                            },
                                        ),
                                    )
                                })
                            })
                            .on_open_change(cx.listener(move |_, open, window, cx| {
                                if !open {
                                    cx.defer_in(window, move |this, window, cx| {
                                        window.focus(&this.meter_focus.unit, cx)
                                    });
                                }
                            })),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("Beats"))
                    .child(
                        div()
                            .id("current-beat")
                            .test_support()
                            .aria_label(status.clone())
                            .text_sm()
                            .child(status),
                    ),
            )
            .child(
                div()
                    .id("beat-grid")
                    .test_support()
                    .grid()
                    .grid_cols(columns)
                    .gap_2()
                    .children((0..meter.beats()).map(|beat| {
                        let owner = owner.clone();
                        let sound = meter.sound(beat);
                        let button = BeatBlock::new(
                            beat,
                            sound,
                            self.position(cx),
                            self.open_beat_menu == Some(beat),
                        )
                        .into_button(cx)
                        .track_focus(&self.meter_focus.beats[usize::from(beat)])
                        .disabled(self.closing)
                        .dropdown_menu(move |menu, _, _| {
                            BeatSound::ALL.into_iter().fold(menu, |menu, choice| {
                                let owner = owner.clone();
                                menu.item(
                                    PopupMenuItem::new(choice.label())
                                        .checked(choice == sound)
                                        .on_click(move |_, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.set_sound(beat, choice, cx)
                                            });
                                        }),
                                )
                            })
                        })
                        .on_open_change(cx.listener(
                            move |this, open, window, cx| {
                                if *open {
                                    this.open_beat_menu = Some(beat);
                                } else if this.open_beat_menu == Some(beat) {
                                    this.open_beat_menu = None;
                                }
                                cx.notify();
                                if !open {
                                    cx.defer_in(window, move |this, window, cx| {
                                        window.focus(&this.meter_focus.beats[usize::from(beat)], cx)
                                    });
                                }
                            },
                        ));
                        anchored(
                            ("beat-scroll-anchor", u32::from(beat + 1)),
                            self.meter_focus.beat_anchors[usize::from(beat)].clone(),
                            button,
                        )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Click a beat to choose its sound. ★ marks the accented first beat."),
            )
    }
}

fn anchored(
    id: impl Into<gpui_kit::ElementId>,
    anchor: gpui_kit::ScrollAnchor,
    child: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(id)
        .anchor_scroll(Some(anchor))
        .flex_1()
        .child(child)
}
