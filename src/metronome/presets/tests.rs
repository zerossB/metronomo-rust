#![expect(clippy::expect_used, reason = "Preset fixtures must be valid.")]

use super::*;

#[test]
fn names_are_trimmed_unique_and_validated_without_altering_the_library_on_failure() {
    let mut library = PresetLibrary::default();
    let id = library
        .add("  Étude  ", Configuration::default())
        .expect("save preset");
    assert_eq!(library.get(id).expect("saved preset").name(), "Étude");
    for name in ["", "   ", "bad\nname", &"a".repeat(65)] {
        assert_eq!(
            library.add(name, Configuration::default()),
            Err(PresetError::InvalidName)
        );
    }
    assert_eq!(
        library.add("ÉTUDE", Configuration::default()),
        Err(PresetError::DuplicateName)
    );
    assert_eq!(library.items().len(), 1);
}

#[test]
fn rename_delete_and_undo_preserve_snapshots_and_never_reuse_ids() {
    let mut library = PresetLibrary::default();
    let id = library
        .add("Practice", Configuration::default())
        .expect("save preset");
    library.rename(id, "Warm-up").expect("rename preset");
    let deleted = library.remove(id).expect("delete preset");
    let newer = library
        .add("Another", Configuration::default())
        .expect("new preset");
    assert!(newer > id);
    library.restore(deleted).expect("undo deletion");
    assert_eq!(library.get(id).expect("restored preset").name(), "Warm-up");
    assert_eq!(
        library.get(id).expect("restored preset").configuration(),
        Configuration::default()
    );
    assert!(library.validate().is_ok());
}

#[test]
fn undo_conflicts_and_capacity_limits_leave_existing_presets_intact() {
    let mut library = PresetLibrary::default();
    let id = library
        .add("Practice", Configuration::default())
        .expect("save preset");
    let deleted = library.remove(id).expect("delete preset");
    library
        .add("Practice", Configuration::default())
        .expect("new same-name preset");
    assert_eq!(library.restore(deleted), Err(PresetError::DuplicateName));
    for ix in 1..MAX_PRESETS {
        library
            .add(&format!("Preset {ix}"), Configuration::default())
            .expect("fill library");
    }
    assert_eq!(
        library.add("One more", Configuration::default()),
        Err(PresetError::Full)
    );
    assert_eq!(library.items().len(), MAX_PRESETS);
}
