use super::*;
use crate::Region::{Cn, Int};
use std::fs;

pub(super) struct TestDirectory(pub(super) PathBuf);

impl TestDirectory {
    pub(super) fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("mzmod-{name}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exported(directory: &str, saved: Settings) -> RegionDraft {
    RegionDraft {
        destination: Destination::Export(directory.into()),
        draft: saved.clone(),
        saved,
        load_error: None,
    }
}

#[test]
fn a_rejected_game_directory_keeps_the_output_and_edits_until_the_next_action() {
    let mut session = EditingSession {
        exe_directory: "portable".into(),
        locations: LocationPreferences::default(),
        drafts: Region::ALL.map(|region| exported(region.code(), Settings::default())),
        applying: None,
        notice: None,
    };
    assert_eq!(session.status(), SessionStatus::ReadyToExport);
    session.edit_page(|region, settings| {
        assert_eq!(region, Region::Int);
        settings.mini_cube = !settings.mini_cube;
    });
    let output = session.output_path();
    let draft = session.current().draft.clone();
    assert_eq!(session.status(), SessionStatus::Unapplied);

    // A file can never pass the game-directory check, even on an installed machine.
    session.select_game_directory(std::env::current_exe().unwrap());
    assert!(session.game_directory().is_none());
    assert_eq!(session.output_path(), output);
    assert_eq!(session.current().draft, draft);
    session.edit_page(|_, _| {});
    assert!(matches!(session.status(), SessionStatus::Error(_)));

    session.edit_page(|_, settings| settings.rune_icons = !settings.rune_icons);
    assert_eq!(session.status(), SessionStatus::Unapplied);
    session.restore_page(Page::Runes);
    assert_eq!(session.current().draft, draft);
    session.select_game_directory(std::env::current_exe().unwrap());
    session.restore_page(Page::Runes);
    assert_eq!(session.status(), SessionStatus::Unapplied);
    session.restore_page(Page::Cube);
    assert_eq!(session.status(), SessionStatus::ReadyToExport);
}

#[test]
fn unreadable_destination_retains_edits_until_the_next_successful_load() {
    let defaults = Settings::default();
    let mut current = exported("first", defaults.clone());
    current.draft.mini_cube = !defaults.mini_cube;
    current.draft.rune_beams[0] = !defaults.rune_beams[0];

    let mut unavailable = exported("second", defaults.clone());
    unavailable.load_error = Some("unreadable".into());
    current.switch_destination(unavailable);
    assert!(current.load_error.is_some());
    assert_eq!(current.destination, Destination::Export("second".into()));
    assert_ne!(current.draft.mini_cube, defaults.mini_cube);
    assert_ne!(current.draft.rune_beams[0], defaults.rune_beams[0]);

    let mut on_disk = defaults.clone();
    on_disk.max_hints = !defaults.max_hints;
    on_disk.rune_beams[1] = !defaults.rune_beams[1];
    current.switch_destination(exported("second", on_disk.clone()));
    assert!(current.load_error.is_none());
    assert_eq!(current.saved, on_disk);
    let mut expected = on_disk;
    expected.mini_cube = !defaults.mini_cube;
    expected.rune_beams[0] = !defaults.rune_beams[0];
    assert_eq!(current.draft, expected);
}

#[test]
fn applying_uses_the_displayed_folder_until_the_player_selects_another() -> Result<()> {
    fn apply_and_wait(session: &mut EditingSession) -> Result<()> {
        let (finished, signal) = std::sync::mpsc::channel();
        session.apply(move || finished.send(()).unwrap());
        signal.recv_timeout(std::time::Duration::from_secs(30))?;
        session.poll_apply();
        Ok(())
    }

    let root = TestDirectory::new("displayed-output");
    let mut session =
        EditingSession::with_locations(root.0.clone(), LocationPreferences::default());
    let shared = Destination::Export(root.0.clone());
    let regional = Destination::Export(root.0.join(Int.code()));
    assert_eq!(session.region(), Int);
    assert_eq!(session.output_path(), shared.live());
    session.edit_page(|_, settings| settings.rune_icons = !settings.rune_icons);
    let edited = session.current().draft.clone();

    // A second window occupies the shared folder after the first displays it.
    let defaults = crate::Settings::default();
    shared.apply(Cn, &defaults)?;
    fs::write(shared.live().join("note.txt"), "keep")?;
    apply_and_wait(&mut session)?;
    assert!(matches!(session.status(), SessionStatus::Error(message)
        if message.contains("请重新选择默认位置。")));
    assert_eq!(session.output_path(), shared.live());
    assert_eq!(session.current().draft, edited);
    assert!(session.has_edits());
    assert!(!regional.live().exists());
    assert_eq!(shared.load(Cn)?, defaults);
    assert_eq!(fs::read(shared.live().join("note.txt"))?, b"keep");

    session.use_default_location();
    assert_eq!(session.output_path(), regional.live());
    assert_eq!(session.status(), SessionStatus::Unapplied);

    // Even if the shared folder becomes available again, this choice still applies.
    fs::rename(shared.live(), root.0.join("moved-cn"))?;
    assert_eq!(Destination::beside(&root.0, Int), shared);
    apply_and_wait(&mut session)?;
    assert_eq!(session.status(), SessionStatus::Exported);
    assert!(!session.has_edits());
    assert_eq!(session.output_path(), regional.live());
    assert_eq!(regional.load(Int)?, edited);
    assert!(!shared.live().exists());
    Ok(())
}
