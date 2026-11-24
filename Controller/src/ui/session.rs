//! Remember output locations and keep each region's unapplied changes across destination switches.
mod locations;

#[cfg(test)]
mod tests;

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
};

use anyhow::{Context, Result};
use locations::{LocationChange, LocationPreferences};

use super::options::Page;
use crate::{install::Destination, Region, Settings};

struct RegionDraft {
    destination: Destination,
    saved: Settings,
    draft: Settings,
    load_error: Option<String>,
}

impl RegionDraft {
    fn open(destination: Destination, region: Region) -> Self {
        let (settings, load_error) = match destination.load(region) {
            Ok(settings) => (settings, None),
            Err(error) => {
                let message = format!("{}：{error:#}", destination.read_context());
                (Settings::default(), Some(message))
            }
        };
        Self {
            destination,
            saved: settings.clone(),
            draft: settings,
            load_error,
        }
    }

    fn switch_destination(&mut self, mut next: Self) {
        if next.load_error.is_some() {
            self.destination = next.destination;
            self.load_error = next.load_error;
            return;
        }
        next.draft.merge_edits(&self.saved, &self.draft);
        *self = next;
    }

    fn has_edits(&self) -> bool {
        self.draft != self.saved
    }
}

enum Notice {
    Applied(Destination),
    Error(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SessionStatus<'a> {
    ReadyToInstall,
    ReadyToExport,
    Applying,
    Unapplied,
    Installed,
    Exported,
    Error(&'a str),
}

struct AppliedSettings {
    region: Region,
    destination: Destination,
    settings: Settings,
}

pub(super) struct EditingSession {
    exe_directory: PathBuf,
    locations: LocationPreferences,
    drafts: [RegionDraft; Region::ALL.len()],
    applying: Option<Receiver<Result<AppliedSettings, String>>>,
    notice: Option<Notice>,
}

impl EditingSession {
    pub(super) fn open() -> Result<Self> {
        let executable = std::env::current_exe().context("找不到设置器路径")?;
        let exe_directory = executable.parent().context("找不到设置器目录")?.to_owned();
        let locations = LocationPreferences::open(&exe_directory);
        Ok(Self::with_locations(exe_directory, locations))
    }

    fn with_locations(exe_directory: PathBuf, locations: LocationPreferences) -> Self {
        let drafts = Region::ALL
            .map(|region| RegionDraft::open(locations.destination(&exe_directory, region), region));
        Self {
            exe_directory,
            locations,
            drafts,
            applying: None,
            notice: None,
        }
    }

    pub(super) fn region(&self) -> Region {
        self.locations.region()
    }

    fn current(&self) -> &RegionDraft {
        &self.drafts[self.region().index()]
    }

    pub(super) fn output_path(&self) -> PathBuf {
        self.current().destination.live()
    }

    pub(super) fn edit_page(&mut self, edit: impl FnOnce(Region, &mut Settings)) {
        let region = self.region();
        let draft = &mut self.drafts[region.index()].draft;
        let previous = draft.clone();
        edit(region, draft);
        if *draft != previous {
            self.notice = None;
        }
    }

    pub(super) fn restore_page(&mut self, page: Page) {
        let index = self.region().index();
        page.restore(&mut self.drafts[index].draft);
        self.notice = None;
    }

    pub(super) fn status(&self) -> SessionStatus<'_> {
        if let Some(notice) = &self.notice {
            return match notice {
                Notice::Error(message) => SessionStatus::Error(message),
                Notice::Applied(Destination::Game(_)) => SessionStatus::Installed,
                Notice::Applied(Destination::Export(_)) => SessionStatus::Exported,
            };
        }
        let current = self.current();
        if let Some(error) = &current.load_error {
            SessionStatus::Error(error)
        } else if self.is_applying() {
            SessionStatus::Applying
        } else if current.has_edits() {
            SessionStatus::Unapplied
        } else {
            match current.destination {
                Destination::Game(_) => SessionStatus::ReadyToInstall,
                Destination::Export(_) => SessionStatus::ReadyToExport,
            }
        }
    }

    pub(super) fn has_edits(&self) -> bool {
        self.drafts.iter().any(RegionDraft::has_edits)
    }

    pub(super) fn game_directory(&self) -> Option<&Path> {
        self.locations.game_directory(self.region())
    }

    pub(super) fn select_region(&mut self, region: Region) {
        self.remember_location(LocationChange::SelectRegion(region));
        self.refresh_destination();
    }

    pub(super) fn select_game_directory(&mut self, path: PathBuf) {
        let region = self.region();
        let next = RegionDraft::open(Destination::Game(path.clone()), region);
        if let Some(error) = next.load_error {
            self.notice = Some(Notice::Error(error));
            return;
        }
        self.drafts[region.index()].switch_destination(next);
        self.remember_location(LocationChange::SetGameDirectory(region, Some(path)));
    }

    pub(super) fn use_default_location(&mut self) {
        self.remember_location(LocationChange::SetGameDirectory(self.region(), None));
        self.refresh_destination();
    }

    fn refresh_destination(&mut self) {
        let region = self.region();
        let destination = self.locations.destination(&self.exe_directory, region);
        self.drafts[region.index()].switch_destination(RegionDraft::open(destination, region));
    }

    fn remember_location(&mut self, change: LocationChange) {
        self.notice = self
            .locations
            .remember(change)
            .err()
            .map(|error| Notice::Error(format!("无法记住此游戏目录：{error:#}")));
    }

    pub(super) fn is_applying(&self) -> bool {
        self.applying.is_some()
    }

    pub(super) fn apply(&mut self, on_finished: impl FnOnce() + Send + 'static) {
        self.notice = None;
        let region = self.region();
        let current = &mut self.drafts[region.index()];
        current.load_error = None;
        let destination = current.destination.clone();
        let baseline = current.saved.clone();
        let edited = current.draft.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = destination
                .apply_edits(region, &baseline, &edited)
                .map(|settings| AppliedSettings {
                    region,
                    destination,
                    settings,
                })
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
            on_finished();
        });
        self.applying = Some(receiver);
    }

    pub(super) fn poll_apply(&mut self) {
        let Some(receiver) = &self.applying else {
            return;
        };
        let outcome = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("设置任务意外中止".into()),
        };
        self.applying = None;
        self.notice = Some(match outcome {
            Ok(applied) => {
                let profile = &mut self.drafts[applied.region.index()];
                if profile.destination == applied.destination {
                    profile.draft.clone_from(&applied.settings);
                    profile.saved = applied.settings;
                }
                Notice::Applied(applied.destination)
            }
            Err(error) => {
                let suggested = self
                    .locations
                    .destination(&self.exe_directory, self.region());
                let hint =
                    if self.game_directory().is_none() && self.current().destination != suggested {
                        "请重新选择默认位置。"
                    } else {
                        ""
                    };
                Notice::Error(format!("应用失败：{hint}{error}"))
            }
        });
    }
}
