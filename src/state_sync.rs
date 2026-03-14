use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt;

use crate::autogenesis::State;
use chrono::{SecondsFormat, Utc};

pub struct StateTransaction {
    lock_file: File,
    state_path: PathBuf,
    tmp_path: PathBuf,
    pub state: State,
}

impl StateTransaction {
    pub fn begin<P: AsRef<Path>>(state_path: P) -> Result<Self> {
        let state_path = state_path.as_ref().to_path_buf();
        let lock_path = state_path.with_extension("lock");
        let tmp_path = state_path.with_extension("tmp");

        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .context("failed to open lock file")?;
        lock_file
            .lock_exclusive()
            .context("failed to acquire exclusive state lock")?;

        let state = load_state_locked(&state_path)?;

        Ok(Self {
            lock_file,
            state_path,
            tmp_path,
            state,
        })
    }

    pub fn commit(mut self) -> Result<()> {
        self.state.updated_at = now_iso();

        let json = serde_json::to_vec_pretty(&self.state)?;
        {
            let mut tmp = File::create(&self.tmp_path).context("failed to create temp state file")?;
            tmp.write_all(&json)?;
            tmp.sync_all()?;
        }

        std::fs::rename(&self.tmp_path, &self.state_path)
            .context("atomic rename failed during state commit")?;

        if let Some(parent) = self.state_path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        self.unlock();
        Ok(())
    }

    pub fn abort(mut self) {
        let _ = std::fs::remove_file(&self.tmp_path);
        self.unlock();
    }

    fn unlock(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

impl Drop for StateTransaction {
    fn drop(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

fn load_state_locked(path: &Path) -> Result<State> {
    if !path.exists() {
        return Ok(State::default());
    }

    let mut file = File::open(path).context("failed to open state file")?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)?;

    if raw.trim().is_empty() {
        return Ok(State::default());
    }

    let mut state: State = serde_json::from_str(&raw).context("failed to parse state json")?;
    if state.version == 0 {
        state.version = 1;
    }
    if state.created_at.is_empty() {
        state.created_at = now_iso();
    }
    if state.updated_at.is_empty() {
        state.updated_at = state.created_at.clone();
    }
    if state.run_lineage.run_id.is_empty() {
        state.run_lineage.run_id = format!("run-{}", uuid::Uuid::new_v4());
    }
    if state.run_lineage.status.is_empty() {
        state.run_lineage.status = "active".to_string();
    }
    Ok(state)
}

pub fn with_state_transaction<P, F, T>(state_path: P, mut f: F) -> Result<T>
where
    P: AsRef<Path>,
    F: FnMut(&mut State) -> Result<T>,
{
    let mut tx = StateTransaction::begin(state_path)?;
    match f(&mut tx.state) {
        Ok(value) => {
            tx.commit()?;
            Ok(value)
        }
        Err(err) => {
            tx.abort();
            Err(err)
        }
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
