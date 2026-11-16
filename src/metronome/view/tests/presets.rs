use super::*;
use crate::metronome::{
    configuration::Configuration,
    meter::{BeatSound, BeatUnit},
    presets::PresetLibrary,
    settings::{Preferences, Settings, SettingsStore},
};

fn open_dialog(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.click("open-presets", cx))
        .expect("open the presets dialog");
    cx.run_until_parked();
}

fn save_named(cx: &mut TestAppContext, handle: AnyWindowHandle, name: &str, keyboard: bool) {
    cx.update_window(handle, |_, window, cx| {
        window.click("preset-name", cx);
        window.press("secondary-a", cx);
        window.input(name, cx);
    })
    .expect("enter a preset name");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        if keyboard {
            window.press("enter", cx);
        } else {
            window.click("save-preset", cx);
        }
    })
    .expect("save a named preset");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn presets_save_with_pointer_and_enter_and_validate_names_without_affecting_transport(
    cx: &mut TestAppContext,
) {
    let (handle, view, device) = open(cx);
    open_dialog(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("empty-presets").visible());
        assert_eq!(window.find("preset-name").focused(), Some(true));
        window.click("save-preset", cx);
    })
    .expect("reject an empty name");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("preset-error").visible());
    })
    .expect("show name validation");
    save_named(cx, handle, "  Test practice  ", false);
    save_named(cx, handle, "Second", true);
    save_named(cx, handle, "TEST PRACTICE", true);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preset-row", 1_u32)).label(),
            Some("Test practice")
        );
        assert_eq!(window.find(("preset-row", 2_u32)).label(), Some("Second"));
        assert_eq!(view.read(cx).session.read(cx).presets().items().len(), 2);
        assert_eq!(
            window.find("preset-error").label(),
            Some("A preset with this name already exists. Choose another name.")
        );
        assert!(view.read(cx).playback(cx) == PlaybackState::Stopped);
        assert_eq!(
            view.read(cx).session.read(cx).taps(),
            0,
            "typing T must not tap tempo"
        );
        window.press("escape", cx);
    })
    .expect("keep validated presets and dismiss with Escape");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("presets-editor").is_none());
        assert_eq!(window.find("open-presets").focused(), Some(true));
        window.press("space", cx);
    })
    .expect("restore focus to the presets button");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("presets-editor").visible());
    })
    .expect("keyboard activation opens presets without starting audio");
    assert_eq!(device.lock().expect("audio state").opens, 0);
}

#[gpui_kit::test]
fn restore_defaults_resets_all_twelve_sounds_and_saved_values_while_preserving_presets(
    cx: &mut TestAppContext,
) {
    let folder = tempfile::tempdir().expect("temporary settings");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let mut library = PresetLibrary::default();
    let saved = Configuration::new(
        187,
        0,
        Meter::default()
            .with_beats(7)
            .with_unit(BeatUnit::Sixteenth)
            .with_sound(11, BeatSound::Low),
    );
    library.add("Keep me", saved).expect("preset fixture");
    store
        .save(
            &Settings::new(saved.bpm(), saved.volume(), saved.meter())
                .with_presets(library.clone()),
        )
        .expect("saved preferences");
    let state = Arc::new(Mutex::new(DeviceState::default()));
    let device = state.clone();
    let factory: AudioFactory = Arc::new(move || Ok(Box::new(TestPlayback(device.clone()))));
    let (handle, view) = open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    start(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.click("restore-defaults", cx);
        assert_eq!(window.find("tempo-value").label(), Some("120 BPM"));
        assert_eq!(window.find("volume-value").label(), Some("60%"));
        assert_eq!(window.find("current-beat").label(), Some("4 / 4"));
        assert_eq!(view.read(cx).meter(cx), Meter::default());
        assert_eq!(view.read(cx).session.read(cx).presets(), &library);
        assert!(view.read(cx).playback(cx) == PlaybackState::Running);
    })
    .expect("restore all defaults in one action");
    cx.run_until_parked();
    let saved = store.load().expect("persisted defaults");
    assert_eq!(
        (saved.bpm(), saved.volume(), saved.meter()),
        (120, 60, Meter::default())
    );
    assert_eq!(saved.presets(), &library);
    {
        let state = state.lock().expect("running audio");
        assert_eq!(
            (state.bpm, state.volume, state.meter),
            (120, 60, Meter::default())
        );
        assert!(state.playing);
    }
    close(cx, handle);
}

#[gpui_kit::test]
fn presets_rename_delete_undo_load_and_survive_reopening_with_all_sound_slots(
    cx: &mut TestAppContext,
) {
    let folder = tempfile::tempdir().expect("temporary settings");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let mut meter = Meter::default().with_beats(7).with_unit(BeatUnit::Eighth);
    for beat in 0..12 {
        meter = meter.with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
    }
    let configuration = Configuration::new(137, 25, meter);
    store
        .save(&Settings::new(137, 25, meter))
        .expect("initial configuration");
    let state = Arc::new(Mutex::new(DeviceState::default()));
    let device = state.clone();
    let factory: AudioFactory = Arc::new(move || {
        device.lock().expect("audio device").opens += 1;
        Ok(Box::new(TestPlayback(device.clone())))
    });
    let (handle, view) =
        open_with_preferences(cx, factory.clone(), Preferences::from_store(store.clone()));
    open_dialog(cx, handle);
    save_named(cx, handle, "Practice", false);
    choose_menu_item(cx, handle, ("preset-actions", 1_u32), "Rename…");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("preset-name").value(), Some("Practice"));
        assert_eq!(window.find("preset-name").focused(), Some(true));
    })
    .expect("focus the existing preset name");
    save_named(cx, handle, "Evening practice", true);
    save_named(cx, handle, "Another", false);
    choose_menu_item(cx, handle, ("preset-actions", 1_u32), "Delete");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("preset-row", 1_u32)).is_none());
        assert_eq!(window.find("preset-name").focused(), Some(true));
        window.click("undo-delete-preset", cx);
    })
    .expect("undo a preset deletion");
    cx.run_until_parked();
    let saved = store.load().expect("saved library");
    assert_eq!(
        saved
            .presets()
            .get(1)
            .expect("restored preset")
            .configuration(),
        configuration
    );
    assert_eq!(
        saved.presets().get(1).expect("renamed preset").name(),
        "Evening practice"
    );
    cx.update_window(handle, |_, window, cx| {
        window.within("preset-dialog-footer").click("close", cx);
    })
    .expect("close the dialog through its native button");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.click("restore-defaults", cx))
        .expect("change the current configuration without deleting the library");
    start(cx, handle);
    open_dialog(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("tab", cx);
        window.press("tab", cx);
        assert_eq!(window.find(("load-preset", 1_u32)).focused(), Some(true));
        window.press("enter", cx);
    })
    .expect("load a saved preset through its focused button");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("presets-editor").is_none());
        assert_eq!(window.find("tempo-value").label(), Some("137 BPM"));
        assert_eq!(window.find("volume-value").label(), Some("25%"));
        assert_eq!(view.read(cx).meter(cx), meter);
        assert_eq!(window.find("open-presets").focused(), Some(true));
        assert!(view.read(cx).playback(cx) == PlaybackState::Running);
    })
    .expect("apply the entire snapshot without stopping playback");
    {
        let device = state.lock().expect("configured audio");
        assert_eq!((device.bpm, device.volume, device.meter), (137, 25, meter));
        assert_eq!(device.opens, 1);
    }
    open_dialog(cx, handle);
    choose_menu_item(cx, handle, ("preset-actions", 2_u32), "Delete");
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))
        .expect("dismiss the dialog");
    cx.run_until_parked();
    close(cx, handle);
    let (handle, reopened) =
        open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(reopened.read(cx).playback(cx) == PlaybackState::Stopped);
        window.click("restore-defaults", cx);
    })
    .expect("restore defaults after reopening");
    open_dialog(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preset-row", 1_u32)).label(),
            Some("Evening practice")
        );
        assert!(window.try_find(("preset-row", 2_u32)).is_none());
        assert!(window.try_find("undo-delete-preset").is_none());
        window.click(("load-preset", 1_u32), cx);
    })
    .expect("load a persisted preset with the pointer");
    cx.run_until_parked();
    assert_eq!(
        store
            .load()
            .expect("persisted loaded configuration")
            .meter(),
        meter
    );
    close(cx, handle);
}

#[gpui_kit::test]
fn preset_menus_restore_focus_and_name_conflicts_do_not_overwrite_snapshots(
    cx: &mut TestAppContext,
) {
    let (handle, view, _) = open(cx);
    open_dialog(cx, handle);
    save_named(cx, handle, "Alpha", false);
    save_named(cx, handle, "Beta", false);
    choose_menu_item(cx, handle, ("preset-actions", 1_u32), "Rename…");
    save_named(cx, handle, "Beta", true);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("preset-row", 1_u32)).label(), Some("Alpha"));
        assert!(window.find("preset-error").visible());
        window.click("cancel-rename", cx);
        window.click(("preset-actions", 1_u32), cx);
    })
    .expect("cancel a conflicting rename and open actions");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))
        .expect("dismiss only the menu");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("preset-actions", 1_u32)).focused(), Some(true));
        assert!(window.find("presets-editor").visible());
        assert!(window.try_find("preset-error").is_none());
    })
    .expect("return keyboard focus to the actions trigger");
    cx.update_window(handle, |_, window, cx| {
        window.click("preset-name", cx);
        window.input("Unfinished name", cx);
    })
    .expect("start naming another preset");
    cx.run_until_parked();
    choose_menu_item(cx, handle, ("preset-actions", 2_u32), "Delete");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("preset-name").value(), Some("Unfinished name"));
    })
    .expect("keep an unrelated draft name when deleting a preset");
    save_named(cx, handle, "Beta", false);
    cx.update_window(handle, |_, window, cx| {
        window.click("undo-delete-preset", cx);
        assert_eq!(view.read(cx).session.read(cx).presets().items().len(), 2);
        assert_eq!(
            view.read(cx)
                .session
                .read(cx)
                .presets()
                .get(3)
                .expect("new preset")
                .name(),
            "Beta"
        );
        assert!(window.find("preset-error").visible());
    })
    .expect("reject an undo that conflicts with a newly saved name");
}

#[gpui_kit::test]
fn preset_save_errors_are_visible_inside_the_dialog_and_recover_without_losing_entries(
    cx: &mut TestAppContext,
) {
    let folder = tempfile::tempdir().expect("temporary settings");
    let blocked = folder.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").expect("blocked config folder");
    let store = SettingsStore::new(blocked.join("settings.json"));
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    open_dialog(cx, handle);
    save_named(cx, handle, "First", false);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("preset-save-error").visible());
        assert_eq!(window.find(("preset-row", 1_u32)).label(), Some("First"));
    })
    .expect("show the real save failure inside the modal");
    std::fs::remove_file(blocked).expect("restore write access");
    save_named(cx, handle, "Second", false);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("preset-save-error").is_none());
        window.press("escape", cx);
    })
    .expect("clear the warning after a successful retry");
    cx.run_until_parked();
    assert_eq!(
        store
            .load()
            .expect("saved library after retry")
            .presets()
            .items()
            .len(),
        2
    );
    close(cx, handle);
}

#[gpui_kit::test]
fn all_presets_and_the_close_button_remain_keyboard_reachable_in_a_zoomed_small_window(
    cx: &mut TestAppContext,
) {
    use gpui_kit::component::Theme;
    let folder = tempfile::tempdir().expect("temporary settings");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let mut library = PresetLibrary::default();
    for ix in 0..crate::metronome::presets::MAX_PRESETS {
        library
            .add(&format!("Practice {ix}"), Configuration::default())
            .expect("fixture preset");
    }
    store
        .save(&Settings::default().with_presets(library))
        .expect("saved library");
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(cx, factory, Preferences::from_store(store));
    cx.update_window(handle, |_, window, _| window.activate_window())
        .expect("activate window");
    cx.simulate_window_resize(handle, size(px(480.), px(720.)));
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(20.)));
    open_dialog(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for _ in 0..129 {
            window.press("tab", cx);
        }
        assert_eq!(
            window.find(("preset-actions", 64_u32)).focused(),
            Some(true)
        );
    })
    .expect("navigate to the last preset");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(window.find(("preset-actions", 64_u32)).visible());
        window.press("tab", cx);
        assert_eq!(
            window
                .within("preset-dialog-footer")
                .find("close")
                .focused(),
            Some(true)
        );
        window.press("enter", cx);
    })
    .expect("reveal the last row and close using the keyboard");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("presets-editor").is_none());
        assert_eq!(window.find("open-presets").focused(), Some(true));
    })
    .expect("restore focus after keyboard dismissal");
    close(cx, handle);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "Uses a real GPU renderer to export preset dialog previews into the temporary directory."]
fn render_presets_previews() {
    use gpui_kit::{
        HeadlessAppContext,
        assets::Assets,
        component::{Theme, ThemeMode},
    };
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::init(cx);
        crate::metronome::init(cx);
    });
    for (name, mode, width, height, font_size) in [
        ("light", ThemeMode::Light, 600., 780., 16.),
        ("dark", ThemeMode::Dark, 600., 780., 16.),
        ("zoom", ThemeMode::Dark, 480., 720., 20.),
    ] {
        let (handle, _) =
            cx.update(|cx| {
                Theme::change(mode, None, cx);
                Theme::update(cx, |theme| theme.font_size = px(font_size));
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(width), px(height)),
                        ))),
                        show: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                        let view = Metronome::with_audio_factory(
                            window,
                            cx,
                            Arc::new(|| Err(AudioError::NoDevice)),
                        );
                        for ix in 0..8 {
                            let label = if ix == 0 {
                                "Evening practice with accents and quiet beats for a long session"
                                    .to_owned()
                            } else {
                                format!("Practice {ix}")
                            };
                            view.session.update(cx, |session, cx| {
                                session.apply_configuration(Configuration::new(
                                    80 + ix * 10, 60, Meter::default().with_beats(3 + ix as u8 % 4),
                                ), cx);
                                session.save_preset(&label, cx)
                            }).expect("preview preset");
                        }
                        view.session.update(cx, |session, cx| session.restore_defaults(cx));
                        view
                    })
                    },
                )
                .expect("open preview")
            });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("open-presets", cx);
        })
        .expect("open presets for preview");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("preset-name").visible());
            assert!(window.find("save-preset").visible());
            assert!(
                window
                    .within("preset-dialog-footer")
                    .find("close")
                    .visible()
            );
            let editor = window.find("presets-editor").bounds();
            assert!(editor.left() >= px(0.) && editor.right() <= px(width));
            window.click("preset-name", cx);
            window.input("Practice 1", cx);
            assert_eq!(window.find("preset-name").value(), Some("Practice 1"));
        })
        .expect("exercise preset validation in the rendered dialog");
        cx.run_until_parked();
        cx.capture_screenshot(handle)
            .expect("render dialog before validation")
            .save(std::env::temp_dir().join(format!("metronomo-presets-{name}-ready.png")))
            .expect("save ready preview");
        cx.update_window(handle, |_, window, cx| window.click("save-preset", cx))
            .expect("submit the preset name");
        cx.run_until_parked();
        cx.capture_screenshot(handle)
            .expect("render dialog after submission")
            .save(std::env::temp_dir().join(format!("metronomo-presets-{name}.png")))
            .expect("save submitted preview");
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("preset-error").visible());
        })
        .expect("keep validation visible");
        cx.capture_screenshot(handle)
            .expect("render presets")
            .save(std::env::temp_dir().join(format!("metronomo-presets-{name}.png")))
            .expect("save preview");
        cx.update_window(handle, |_, window, _| window.remove_window())
            .expect("close preview");
    }
}
