//! Preset commands share the metronome owner; the modal owns its name editor and focus.

use std::{cell::Cell, collections::BTreeMap};

use gpui_kit::component::{
    WindowExt as _,
    dialog::{DialogClose, DialogEntrance, DialogFooter},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{AnyElement, Focusable as _, NoAction, ScrollAnchor, ScrollHandle, WeakEntity};

use super::super::presets::{Preset, PresetError};
use super::*;

pub(super) fn init(cx: &mut App) {
    // Keep Enter on a row button from invoking the modal's default Confirm action.
    cx.bind_keys([KeyBinding::new(
        "enter",
        NoAction {},
        Some("PresetsDialog && !Input && !PopupMenu"),
    )]);
}

impl Metronome {
    pub(super) fn render_preset_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .key_context("PresetControls")
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .px_4()
            .py_2()
            .child(
                Button::new("open-presets")
                    .track_focus(&self.preset_focus)
                    .label("Presets…")
                    .outline()
                    .small()
                    .disabled(self.closing)
                    .on_click(cx.listener(|this, _, window, cx| this.open_presets(window, cx))),
            )
            .child(
                Button::new("restore-defaults")
                    .label("Restore defaults")
                    .ghost()
                    .small()
                    .disabled(self.closing)
                    .tooltip("Restore 120 BPM, 60% volume, 4/4, and the default beat sounds")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.apply_configuration(Configuration::default(), window, cx)
                    })),
            )
    }

    pub(super) fn apply_configuration(
        &mut self,
        configuration: Configuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closing {
            return;
        }
        self.reset_taps();
        let meter = configuration.meter();
        if meter.beats() != self.meter.beats() || meter.unit() != self.meter.unit() {
            self.position = PlaybackPosition::idle();
        }
        self.meter = meter;
        self.tempo.update(cx, |tempo, cx| {
            tempo.set_value(f32::from(configuration.bpm()), window, cx)
        });
        self.volume.update(cx, |volume, cx| {
            volume.set_value(f32::from(configuration.volume()), window, cx)
        });
        self.configure_audio(cx);
        self.save_preferences(cx);
        cx.notify();
    }

    fn configuration(&self, cx: &App) -> Configuration {
        Configuration::new(self.bpm(cx) as u16, self.volume(cx) as u8, self.meter)
    }

    fn open_presets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        window.focus(&self.preset_focus, cx);
        let owner = cx.entity();
        let editor =
            cx.new(|cx| PresetsDialog::new(owner.downgrade(), self.settings_error, window, cx));
        let ids = self.presets.items().iter().map(Preset::id).collect();
        editor.update(cx, |editor, cx| editor.sync_row_focus(ids, window, cx));
        let initial_focus = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                editor.read(cx).name.focus_handle(cx).focus(window, cx);
            }
            let content = editor.clone();
            dialog
                .title("Presets")
                .width(window.rem_size() * 28.)
                .entrance(DialogEntrance::None)
                .on_ok(|_, _, _| false)
                .content(move |content_box, _, _| content_box.min_h_0().child(content.clone()))
                .footer(
                    div()
                        .id("preset-dialog-footer")
                        .key_context("PresetsDialog")
                        .child(DialogFooter::new().child(
                            DialogClose::new().trigger(|button| button.label("Close").outline()),
                        )),
                )
        });
    }
}

struct RowFocus {
    load: FocusHandle,
    actions: FocusHandle,
    anchor: ScrollAnchor,
    _subscriptions: Vec<Subscription>,
}

struct PresetsDialog {
    owner: WeakEntity<Metronome>,
    name: Entity<InputState>,
    renaming: Option<u32>,
    error: Option<String>,
    deleted: Option<Preset>,
    focus_name_after_menu: bool,
    scroll: ScrollHandle,
    form_anchor: ScrollAnchor,
    row_focus: BTreeMap<u32, RowFocus>,
    last_settings_error: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl PresetsDialog {
    fn new(
        owner: WeakEntity<Metronome>,
        settings_error: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Evening practice"));
        let subscription =
            cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.save(window, cx),
                InputEvent::Change => {
                    this.error = None;
                    cx.notify();
                }
                _ => {}
            });
        let scroll = ScrollHandle::default();
        let form_anchor = ScrollAnchor::for_handle(scroll.clone());
        let focus = name.focus_handle(cx);
        let anchor = form_anchor.clone();
        let focus_subscription = cx.on_focus(&focus.clone(), window, move |_, window, cx| {
            if window.last_input_was_keyboard() && focus.is_focused(window) {
                anchor.scroll_to(window, cx);
                cx.notify();
            }
        });
        let mut subscriptions = vec![subscription, focus_subscription];
        if let Some(owner) = owner.upgrade() {
            subscriptions.push(cx.observe(&owner, |this, owner, cx| {
                let error = owner.read(cx).settings_error;
                if error != this.last_settings_error {
                    this.last_settings_error = error;
                    cx.notify();
                }
            }));
        }
        Self {
            owner,
            name,
            renaming: None,
            error: None,
            deleted: None,
            focus_name_after_menu: false,
            scroll,
            form_anchor,
            row_focus: BTreeMap::new(),
            last_settings_error: settings_error,
            _subscriptions: subscriptions,
        }
    }

    fn sync_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let ids: Vec<_> = owner
            .read(cx)
            .presets
            .items()
            .iter()
            .map(Preset::id)
            .collect();
        self.sync_row_focus(ids, window, cx);
    }

    fn sync_row_focus(&mut self, ids: Vec<u32>, window: &mut Window, cx: &mut Context<Self>) {
        self.row_focus.retain(|id, _| ids.contains(id));
        for id in ids {
            self.row_focus.entry(id).or_insert_with(|| {
                let load = cx.focus_handle().tab_stop(true);
                let actions = cx.focus_handle().tab_stop(true);
                let anchor = ScrollAnchor::for_handle(self.scroll.clone());
                let subscriptions = [&load, &actions]
                    .into_iter()
                    .map(|focus| {
                        let target = focus.clone();
                        let anchor = anchor.clone();
                        cx.on_focus(focus, window, move |_, window, cx| {
                            if window.last_input_was_keyboard() && target.is_focused(window) {
                                anchor.scroll_to(window, cx);
                                cx.notify();
                            }
                        })
                    })
                    .collect();
                RowFocus {
                    load,
                    actions,
                    anchor,
                    _subscriptions: subscriptions,
                }
            });
        }
    }

    fn focus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |name, cx| name.focus(window, cx));
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().to_string();
        let result = self
            .owner
            .update(cx, |owner, cx| {
                let result = if let Some(id) = self.renaming {
                    owner.presets.rename(id, &name)
                } else {
                    let configuration = owner.configuration(cx);
                    owner.presets.add(&name, configuration).map(|_| ())
                };
                if result.is_ok() {
                    owner.save_preferences(cx);
                    cx.notify();
                }
                result
            })
            .unwrap_or(Err(PresetError::NotFound));
        match result {
            Ok(()) => {
                self.renaming = None;
                self.name
                    .update(cx, |name, cx| name.set_value("", window, cx));
                self.error = None;
                self.sync_focus(window, cx);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.focus_name(window, cx);
        cx.notify();
    }

    fn rename(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.owner.upgrade().and_then(|owner| {
            owner
                .read(cx)
                .presets
                .get(id)
                .map(|preset| preset.name().to_owned())
        });
        if let Some(name) = name {
            self.renaming = Some(id);
            self.error = None;
            self.focus_name_after_menu = true;
            self.name
                .update(cx, |input, cx| input.set_value(name, window, cx));
            cx.notify();
        }
    }

    fn delete(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let deleted = self
            .owner
            .update(cx, |owner, cx| {
                let deleted = owner.presets.remove(id);
                if deleted.is_some() {
                    owner.save_preferences(cx);
                    cx.notify();
                }
                deleted
            })
            .ok()
            .flatten();
        if let Some(deleted) = deleted {
            self.deleted = Some(deleted);
            self.focus_name_after_menu = true;
            self.error = None;
            if self.renaming == Some(id) {
                self.renaming = None;
                self.name
                    .update(cx, |name, cx| name.set_value("", window, cx));
            }
            self.sync_focus(window, cx);
            // The deleted row's popover may unmount before it emits its close event.
            cx.defer_in(window, |this, window, cx| {
                this.focus_name_after_menu = false;
                this.focus_name(window, cx);
            });
            cx.notify();
        }
    }

    fn undo_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preset) = self.deleted.clone() else {
            return;
        };
        let result = self
            .owner
            .update(cx, |owner, cx| {
                let result = owner.presets.restore(preset);
                if result.is_ok() {
                    owner.save_preferences(cx);
                    cx.notify();
                }
                result
            })
            .unwrap_or(Err(PresetError::NotFound));
        match result {
            Ok(()) => {
                self.deleted = None;
                self.error = None;
                self.sync_focus(window, cx);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn load(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let loaded = self
            .owner
            .update(cx, |owner, cx| {
                let configuration = owner.presets.get(id).map(Preset::configuration);
                if let Some(configuration) = configuration {
                    owner.apply_configuration(configuration, window, cx);
                }
                configuration.is_some()
            })
            .unwrap_or(false);
        if loaded {
            window.close_dialog(cx);
        }
    }

    fn render_row(&self, preset: &Preset, cx: &Context<Self>) -> AnyElement {
        let id = preset.id();
        let Some(focus) = self.row_focus.get(&id) else {
            return div().into_any_element();
        };
        let owner = cx.entity().downgrade();
        let rename_owner = owner.clone();
        div()
            .id(("preset-row", id))
            .test_support()
            .aria_label(preset.name().to_owned())
            .anchor_scroll(Some(focus.anchor.clone()))
            .flex()
            .items_center()
            .gap_2()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().truncate().text_sm().child(preset.name().to_owned()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(summary(preset.configuration())),
                    ),
            )
            .child(
                Button::new(("load-preset", id))
                    .track_focus(&focus.load)
                    .label("Load")
                    .outline()
                    .small()
                    .accessibility_label(format!("Load {}", preset.name()))
                    .tooltip(format!("Load {}", preset.name()))
                    .on_click(cx.listener(move |this, _, window, cx| this.load(id, window, cx))),
            )
            .child(
                Button::new(("preset-actions", id))
                    .track_focus(&focus.actions)
                    .label("Actions")
                    .ghost()
                    .small()
                    .accessibility_label(format!("Actions for {}", preset.name()))
                    .dropdown_menu(move |menu, _, _| {
                        let owner = owner.clone();
                        let rename_owner = rename_owner.clone();
                        menu.item(
                            PopupMenuItem::new("Rename…").on_click(move |_, window, cx| {
                                let _ =
                                    rename_owner.update(cx, |this, cx| this.rename(id, window, cx));
                            }),
                        )
                        .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| this.delete(id, window, cx));
                        }))
                    })
                    .on_open_change(cx.listener(move |this, open, window, cx| {
                        if !open {
                            let focus_name = std::mem::take(&mut this.focus_name_after_menu);
                            cx.defer_in(window, move |this, window, cx| {
                                if !focus_name && let Some(focus) = this.row_focus.get(&id) {
                                    window.focus(&focus.actions, cx);
                                } else {
                                    this.focus_name(window, cx);
                                }
                            });
                        }
                    })),
            )
            .into_any_element()
    }
}

impl Render for PresetsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            return div().into_any_element();
        };
        let owner = owner.read(cx);
        let content = div()
            .flex()
            .flex_col()
            .gap_4()
            .p_1()
            .child(self.render_form(owner.configuration(cx), owner.settings_error, cx))
            .children(self.deleted.as_ref().map(|preset| {
                div()
                    .id("deleted-preset")
                    .test_support()
                    .aria_label(format!("Deleted {}", preset.name()))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Deleted “{}”", preset.name())),
                    )
                    .child(
                        Button::new("undo-delete-preset")
                            .label("Undo delete")
                            .outline()
                            .small()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.undo_delete(window, cx)),
                            ),
                    )
            }))
            .child(div().text_sm().child("Saved presets"))
            .children(owner.presets.items().is_empty().then(|| {
                div()
                    .id("empty-presets")
                    .test_support()
                    .aria_label("No saved presets")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No saved presets. Name your current settings to save one.")
            }))
            .children(
                owner
                    .presets
                    .items()
                    .iter()
                    .map(|preset| self.render_row(preset, cx)),
            );

        div()
            .id("presets-editor")
            .test_support()
            .key_context("PresetsDialog")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .max_h(gpui_kit::rems(22.))
            .child(
                div()
                    .id("preset-scroll")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .vertical_scrollbar(&self.scroll)
                    .child(content),
            )
            .into_any_element()
    }
}

impl PresetsDialog {
    fn render_form(
        &self,
        current: Configuration,
        save_error: Option<&'static str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let actions = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("save-preset")
                    .label(if self.renaming.is_some() {
                        "Rename"
                    } else {
                        "Save preset"
                    })
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            )
            .children(self.renaming.map(|_| {
                Button::new("cancel-rename")
                    .label("Cancel rename")
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.renaming = None;
                        this.error = None;
                        this.name
                            .update(cx, |name, cx| name.set_value("", window, cx));
                        this.focus_name(window, cx);
                        cx.notify();
                    }))
            }));
        div()
            .id("preset-form")
            .anchor_scroll(Some(self.form_anchor.clone()))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Current: {}", summary(current))),
            )
            .child(div().text_sm().child(if self.renaming.is_some() {
                "New name"
            } else {
                "Preset name"
            }))
            .child(
                Input::new(&self.name)
                    .id("preset-name")
                    .aria_label("Preset name")
                    .w_full(),
            )
            .child(actions)
            .children(self.error.as_ref().map(|message| {
                div()
                    .id("preset-error")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(message.clone())
            }))
            .children(save_error.map(|message| {
                div()
                    .id("preset-save-error")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(message)
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(message)
            }))
            .into_any_element()
    }
}

fn summary(configuration: Configuration) -> String {
    let meter = configuration.meter();
    format!(
        "{} BPM · {}/{} · {}%",
        configuration.bpm(),
        meter.beats(),
        meter.unit().denominator(),
        configuration.volume()
    )
}
