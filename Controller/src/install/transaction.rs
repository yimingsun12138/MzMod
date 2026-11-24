//! Hold the install lock while preparing, replacing or recovering an installation.
use super::{existing::ExistingInstallation, ordinary_metadata, path_exists, MOD_DIRECTORY};
use anyhow::{ensure, Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

const WORK_DIRECTORY: &str = ".mzmod-controller";
const PREVIOUS: &str = "previous";
const STAGING: &str = "staging";

pub(super) fn previous_install(parent: &Path) -> PathBuf {
    parent.join(WORK_DIRECTORY).join(PREVIOUS)
}

pub(super) struct Workspace {
    pub(super) live: PathBuf,
    stage: PathBuf,
    previous: PathBuf,
    _lock: File,
}

impl Workspace {
    /// An initial read needs no work directory. Existing installs recover under the lock.
    pub(super) fn open_existing(parent: &Path) -> Result<Option<Self>> {
        if path_exists(&parent.join(WORK_DIRECTORY))? {
            Ok(Some(Self::open(parent)?))
        } else {
            Ok(None)
        }
    }

    pub(super) fn open(parent: &Path) -> Result<Self> {
        fs::create_dir_all(parent)?;
        ordinary_metadata(parent)?;
        let work = parent.join(WORK_DIRECTORY);
        fs::create_dir_all(&work)?;
        ordinary_metadata(&work)?;
        let lock_path = work.join("install.lock");
        let stage = work.join(STAGING);
        let previous = work.join(PREVIOUS);
        if path_exists(&lock_path)? {
            ordinary_metadata(&lock_path)?;
        } else {
            ensure!(
                !path_exists(&stage)? && !path_exists(&previous)?,
                "此位置已有同名工作目录，请保留其中的文件后再应用：{}",
                work.display()
            );
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock().context("另一项设置操作正在使用这个目录")?;
        let workspace = Self {
            live: parent.join(MOD_DIRECTORY),
            stage,
            previous,
            _lock: lock,
        };
        workspace.recover()?;
        Ok(workspace)
    }

    fn recover(&self) -> Result<()> {
        // Between the two renames, the previous directory is the last committed state.
        if path_exists(&self.previous)? {
            ordinary_metadata(&self.previous)?;
            if path_exists(&self.live)? {
                ExistingInstallation::verify_committed(&self.live).with_context(|| {
                    format!(
                        "检测到中断的更新，旧目录已保存在{}",
                        self.previous.display()
                    )
                })?;
                fs::remove_dir_all(&self.previous)?;
            } else {
                fs::rename(&self.previous, &self.live)?;
            }
        }
        if path_exists(&self.stage)? {
            ordinary_metadata(&self.stage)?;
            fs::remove_dir_all(&self.stage)?;
        }
        Ok(())
    }

    pub(super) fn replace(
        self,
        write_stage: impl FnOnce(&Path, &Path) -> Result<()>,
    ) -> Result<()> {
        fs::create_dir(&self.stage)?;
        write_stage(&self.live, &self.stage)?;
        self.commit()
    }

    fn commit(&self) -> Result<()> {
        let had_install = path_exists(&self.live)?;
        if had_install {
            fs::rename(&self.live, &self.previous).context("请关闭游戏后再应用设置")?;
        }
        if let Err(error) = fs::rename(&self.stage, &self.live) {
            if had_install {
                fs::rename(&self.previous, &self.live).context("恢复原安装目录失败")?;
            }
            return Err(error.into());
        }
        // A locked backup can be collected when the controller next opens the install.
        if had_install {
            let _ = fs::remove_dir_all(&self.previous);
        }
        Ok(())
    }
}
