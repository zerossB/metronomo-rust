//! Coalesces preference snapshots without performing file I/O on the UI thread.

use super::{Settings, SettingsStore};
use gpui_kit::{AppContext as _, Context, Subscription, Task};

pub(in crate::metronome) struct PreferencesWriter {
    store: Option<SettingsStore>,
    error: Option<&'static str>,
    latest: Option<Settings>,
    pending: Option<Settings>,
    dirty: bool,
    task: Option<Task<()>>,
    _quit: Subscription,
}

impl PreferencesWriter {
    pub(in crate::metronome) fn new(
        store: Option<SettingsStore>,
        error: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> Self {
        let quit = cx.on_app_quit(|this, cx| {
            // GPUI destroys windows before polling quit futures. Capture the final
            // snapshot now; the write must not depend on any foreground entity.
            let save = this
                .store
                .clone()
                .filter(|_| this.dirty)
                .zip(this.latest.clone())
                .map(|(store, settings)| {
                    store.begin_shutdown();
                    this.task = None;
                    cx.background_spawn(async move {
                        if let Err(error) = store.save_on_shutdown(&settings) {
                            eprintln!("Could not save Metronomo settings during shutdown: {error}");
                        }
                    })
                });
            async move {
                if let Some(save) = save {
                    save.await;
                }
            }
        });
        Self {
            store,
            error,
            latest: None,
            pending: None,
            dirty: false,
            task: None,
            _quit: quit,
        }
    }

    pub(in crate::metronome) fn error(&self) -> Option<&'static str> {
        self.error
    }

    pub(in crate::metronome) fn enqueue(&mut self, settings: Settings, cx: &mut Context<Self>) {
        if self.store.is_none() {
            return;
        }
        self.latest = Some(settings.clone());
        self.pending = Some(settings);
        self.dirty = true;
        self.start(cx);
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.task.is_some() {
            return;
        }
        self.task = Some(cx.spawn(async move |this, cx| {
            loop {
                let next = this
                    .update(cx, |this, _| {
                        Some((this.store.clone()?, this.pending.take()?))
                    })
                    .ok()
                    .flatten();
                let Some((store, settings)) = next else {
                    break;
                };
                let result = cx
                    .background_spawn(async move { store.save(&settings) })
                    .await;
                let keep_saving = this
                    .update(cx, |this, cx| {
                        let saved = result.is_ok();
                        this.error = match result {
                            Ok(()) => None,
                            Err(error) => {
                                eprintln!("Could not save Metronomo settings: {error}");
                                Some("Couldn’t save settings. Check access to your configuration folder.")
                            }
                        };
                        let pending = this.pending.is_some();
                        if !pending {
                            this.task = None;
                            this.dirty = !saved;
                        }
                        cx.notify();
                        pending
                    })
                    .unwrap_or(false);
                if !keep_saving {
                    break;
                }
            }
        }));
    }

    /// Retry a failed save once on close, then await the entire pending queue.
    pub(in crate::metronome) fn finish(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if self.dirty && self.task.is_none() {
            self.pending = self.latest.clone();
            self.start(cx);
        }
        let task = self.task.take();
        let owner = cx.entity();
        cx.spawn(async move |_, _| {
            if let Some(task) = task {
                task.await;
            }
            drop(owner);
        })
    }
}
