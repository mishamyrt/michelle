//! Ordered, coalesced writes of desktop preferences and window state.

use std::cell::Cell;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use smol::channel::{Receiver, Sender};

use super::{AppSettings, AppState, read_app_state_file, write_json_atomically};

struct Snapshot {
    generation: u64,
    settings: AppSettings,
    state: AppState,
}

pub(super) struct AppFileSaveQueue {
    updates: Sender<Snapshot>,
    errors: Receiver<io::Error>,
    generation: Cell<u64>,
    completed: Arc<AtomicU64>,
    completion_wake: Receiver<()>,
}

impl AppFileSaveQueue {
    pub(super) fn save(&self, settings: AppSettings, state: AppState) -> io::Result<()> {
        let generation = self.generation.get() + 1;
        self.updates
            .try_send(Snapshot {
                generation,
                settings,
                state,
            })
            .map_err(|_| io::Error::other("desktop state writer is closed"))?;
        self.generation.set(generation);
        Ok(())
    }

    pub(super) fn errors(&self) -> Receiver<io::Error> {
        self.errors.clone()
    }

    pub(super) fn wait(&self) -> impl Future<Output = ()> + Send + 'static + use<> {
        let generation = self.generation.get();
        let completed = self.completed.clone();
        let wake = self.completion_wake.clone();
        async move {
            while completed.load(Ordering::Acquire) < generation {
                if wake.recv().await.is_err() {
                    break;
                }
            }
        }
    }
}

/// The sole consumer of local snapshots. Its future belongs on a background
/// executor: reads, serialization and atomic writes all execute there.
pub struct AppFileWriter {
    settings_path: PathBuf,
    state_path: PathBuf,
    updates: Receiver<Snapshot>,
    errors: Sender<io::Error>,
    completed: Arc<AtomicU64>,
    completion_wake: Sender<()>,
    loaded: bool,
    settings: Option<AppSettings>,
    state: Option<AppState>,
}

impl AppFileWriter {
    pub(super) fn channel(settings_path: PathBuf, state_path: PathBuf) -> (AppFileSaveQueue, Self) {
        let (updates, requests) = smol::channel::unbounded();
        let (errors, error_events) = smol::channel::unbounded();
        let (completion_wake, completion_events) = smol::channel::bounded(1);
        let completed = Arc::new(AtomicU64::new(0));
        (
            AppFileSaveQueue {
                updates,
                errors: error_events,
                generation: Cell::new(0),
                completed: completed.clone(),
                completion_wake: completion_events,
            },
            Self {
                settings_path,
                state_path,
                updates: requests,
                errors,
                completed,
                completion_wake,
                loaded: false,
                settings: None,
                state: None,
            },
        )
    }

    pub async fn run(mut self) {
        while let Some(snapshot) = self.next().await {
            let generation = snapshot.generation;
            if let Err(error) = self.write(snapshot) {
                let _ = self.errors.try_send(error);
            }
            // Completion never clears live task dirty flags. A quit waiter
            // must ignore an older write when a newer snapshot is queued.
            self.completed.store(generation, Ordering::Release);
            let _ = self.completion_wake.try_send(());
        }
    }

    async fn next(&self) -> Option<Snapshot> {
        let mut snapshot = self.updates.recv().await.ok()?;
        while let Ok(newer) = self.updates.try_recv() {
            snapshot = newer;
        }
        Some(snapshot)
    }

    fn write(&mut self, snapshot: Snapshot) -> io::Result<()> {
        if !self.loaded {
            self.settings = std::fs::read(&self.settings_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok());
            self.state = read_app_state_file(&self.state_path);
            self.loaded = true;
        }
        if self.settings.as_ref() != Some(&snapshot.settings) {
            write_json_atomically(&self.settings_path, &snapshot.settings)?;
            self.settings = Some(snapshot.settings);
        }
        if self.state.as_ref() != Some(&snapshot.state) {
            write_json_atomically(&self.state_path, &snapshot.state)?;
            self.state = Some(snapshot.state);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::PersistedState;

    fn directory() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("michelle-app-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn enqueue_does_no_io_and_pending_snapshots_coalesce() {
        let root = directory();
        let settings_path = root.join("settings.json");
        let state_path = root.join("state.json");
        let (queue, writer) = AppFileWriter::channel(settings_path.clone(), state_path.clone());
        let mut state = PersistedState::empty();
        queue.save(state.app_settings(), state.app_state()).unwrap();
        state.render_math = false;
        state.sidebar_width = 310.0;
        queue.save(state.app_settings(), state.app_state()).unwrap();

        assert!(!settings_path.exists());
        assert!(!state_path.exists());
        let latest = smol::block_on(writer.next()).unwrap();
        assert_eq!(latest.generation, 2);
        assert!(!latest.settings.render_math);
        assert_eq!(latest.state.sidebar_width, 310.0);
        assert!(writer.updates.try_recv().is_err());

        // A newer snapshot also survives closure of the owner during shutdown.
        queue.save(state.app_settings(), state.app_state()).unwrap();
        let completed = queue.completed.clone();
        drop(queue);
        smol::block_on(writer.run());
        assert_eq!(completed.load(Ordering::Acquire), 3);
        assert_eq!(
            read_app_state_file(&state_path).unwrap().sidebar_width,
            310.0
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unchanged_files_are_not_replaced_and_each_file_changes_independently() {
        use std::os::unix::fs::MetadataExt;

        let root = directory();
        let settings_path = root.join("settings.json");
        let state_path = root.join("state.json");
        let (queue, mut writer) = AppFileWriter::channel(settings_path.clone(), state_path.clone());
        let mut state = PersistedState::empty();
        // Match the files initialized or read by StateStore at launch.
        write_json_atomically(&settings_path, &state.app_settings()).unwrap();
        write_json_atomically(&state_path, &state.app_state()).unwrap();
        let settings_inode = std::fs::metadata(&settings_path).unwrap().ino();
        let state_inode = std::fs::metadata(&state_path).unwrap().ino();

        queue.save(state.app_settings(), state.app_state()).unwrap();
        writer
            .write(smol::block_on(writer.next()).unwrap())
            .unwrap();
        assert_eq!(
            std::fs::metadata(&settings_path).unwrap().ino(),
            settings_inode
        );
        assert_eq!(std::fs::metadata(&state_path).unwrap().ino(), state_inode);

        state.sidebar_width = 310.0;
        queue.save(state.app_settings(), state.app_state()).unwrap();
        writer
            .write(smol::block_on(writer.next()).unwrap())
            .unwrap();
        assert_eq!(
            std::fs::metadata(&settings_path).unwrap().ino(),
            settings_inode
        );
        let changed_state_inode = std::fs::metadata(&state_path).unwrap().ino();
        assert_ne!(changed_state_inode, state_inode);

        state.render_math = false;
        queue.save(state.app_settings(), state.app_state()).unwrap();
        writer
            .write(smol::block_on(writer.next()).unwrap())
            .unwrap();
        assert_ne!(
            std::fs::metadata(&settings_path).unwrap().ino(),
            settings_inode
        );
        assert_eq!(
            std::fs::metadata(&state_path).unwrap().ino(),
            changed_state_inode
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn errors_reach_the_owner_and_failed_writes_retry_before_quit_completes() {
        let root = directory();
        let blocked = root.join("blocked");
        std::fs::write(&blocked, "not a directory").unwrap();
        let settings_path = root.join("settings.json");
        let state_path = blocked.join("state.json");
        let (queue, writer) = AppFileWriter::channel(settings_path, state_path.clone());
        let mut state = PersistedState::empty();
        queue.save(state.app_settings(), state.app_state()).unwrap();
        assert!(smol::block_on(smol::future::poll_once(queue.wait())).is_none());
        let worker = std::thread::spawn(move || smol::block_on(writer.run()));

        smol::block_on(queue.wait());
        assert!(queue.errors.try_recv().is_ok());
        assert!(!state_path.exists());
        std::fs::remove_file(&blocked).unwrap();
        state.sidebar_width = 310.0;
        queue.save(state.app_settings(), state.app_state()).unwrap();
        smol::block_on(queue.wait());

        assert!(queue.errors.try_recv().is_err());
        assert_eq!(queue.completed.load(Ordering::Acquire), 2);
        assert_eq!(
            read_app_state_file(&state_path).unwrap().sidebar_width,
            310.0
        );
        drop(queue);
        worker.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
