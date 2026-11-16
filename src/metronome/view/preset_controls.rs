//! Window toolbar and composition of the preset dialog.
use super::super::presets::Preset;
use super::Metronome;
use super::presets_dialog::PresetsDialog;
use gpui_kit::component::{
    Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::component::{
    WindowExt as _,
    dialog::{DialogClose, DialogEntrance, DialogFooter},
};
use gpui_kit::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, Window, div,
};
use std::cell::Cell;

pub(super) use super::presets_dialog::init;

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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session
                            .update(cx, |session, cx| session.restore_defaults(cx))
                    })),
            )
    }

    fn open_presets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        window.focus(&self.preset_focus, cx);
        let owner = self.session.clone();
        let editor = cx.new(|cx| PresetsDialog::new(owner.downgrade(), window, cx));
        let ids = self
            .session
            .read(cx)
            .presets()
            .items()
            .iter()
            .map(Preset::id)
            .collect();
        editor.update(cx, |editor, cx| editor.sync_row_focus(ids, window, cx));
        let initial_focus = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                editor.read(cx).name_focus(cx).focus(window, cx);
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
