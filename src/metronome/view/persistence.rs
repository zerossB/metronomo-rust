use gpui_kit::{AppContext as _, Context, Window};

use super::{Metronome, PlaybackPosition, PlaybackState, Settings};

impl Metronome {
    pub(super) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        if self.settings_store.is_none() {
            return;
        }
        self.settings_dirty = true;
        self.pending_settings = Some(Settings::new(
            self.bpm(cx) as u16,
            self.volume(cx) as u8,
            self.meter,
        ));
        if self.save_task.is_some() || self.closing {
            return;
        }

        // One writer drains the most recent snapshot. Changes during a write coalesce,
        // so a slower old save can never overwrite a newer one.
        self.save_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let next = this.update(cx, |this, _| {
                    Some((this.settings_store.clone()?, this.pending_settings.take()?))
                }).ok().flatten();
                let Some((store, settings)) = next else { break; };
                let result = cx.background_spawn(async move { store.save(&settings) }).await;
                let keep_saving = this.update(cx, |this, cx| {
                    let saved = result.is_ok();
                    this.settings_error = match result {
                        Ok(()) => None,
                        Err(error) => {
                            eprintln!("Could not save Metronomo settings: {error}");
                            Some("Couldn’t save settings. Check access to your configuration folder.")
                        }
                    };
                    let pending = this.pending_settings.is_some();
                    if !pending {
                        this.save_task = None;
                        this.settings_dirty = !saved;
                    }
                    cx.notify();
                    pending
                }).unwrap_or(false);
                if !keep_saving { break; }
            }
        }));
    }

    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if self.settings_dirty && self.save_task.is_none() {
            self.save_preferences(cx);
        }
        self.closing = true;
        self._startup = None;
        self._monitor = None;
        self.playback = PlaybackState::Stopped;
        self.position = PlaybackPosition::idle();
        self.configure_audio(cx);
        let save = self.save_task.take();
        self.close_task = Some(cx.spawn_in(window, async move |this, cx| {
            if let Some(save) = save {
                save.await;
            }
            let _ = this.update_in(cx, |_, window, _| window.remove_window());
        }));
        cx.notify();
    }
}
