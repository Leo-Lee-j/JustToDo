// JustToDo 存储层：Rust 侧单一数据源
// - data.json / config.json 持久化
// - Parking_lot Mutex 内存缓存
// - 每日备份 + 彻底删除前备份
// - 单实例文件锁

use crate::models::{AppData, Config, APP_VERSION};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct Store {
    data: Mutex<AppData>,
    config: Mutex<Config>,
    dir: PathBuf,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: String,
    pub saved_at: String,
    pub active_tasks: usize,
    pub trashed_tasks: usize,
}

pub enum StartupRecovery {
    Tasks(Backup),
    Settings { has_backup: bool },
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct StickyPosition {
    pub x: i32,
    pub y: i32,
}

impl Store {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(dir.join("backup")).map_err(|err| file_error(&dir, err))?;
        let data_path = dir.join("data.json");
        let mut data: AppData = load_or_default(&data_path)?;
        let config_path = dir.join("config.json");
        let mut config: Config = load_or_default(&config_path)?;
        let backup = dir
            .join("backup")
            .join(format!("daily-{}.json", today_str()));
        if data_path.exists() && !backup.exists() {
            write_json(&backup, &data)?;
            prune_backups(&dir, "daily-");
        }
        // Convert the legacy 1-5 star scale once. Legacy data is identifiable
        // by priority 5; the new scale is 0-4 with 0 as the highest priority.
        if migrate_priorities(&mut data) {
            write_json(&data_path, &data)?;
        }
        if config.version != APP_VERSION {
            config.version = APP_VERSION.into();
            write_json(&config_path, &config)?;
        }

        Ok(Store {
            data: Mutex::new(data),
            config: Mutex::new(config),
            dir,
        })
    }

    pub fn data(&self) -> AppData {
        self.data.lock().clone()
    }

    pub fn config(&self) -> Config {
        self.config.lock().clone()
    }

    pub fn set_config(&self, config: Config) -> Result<(), String> {
        let mut c = self.config.lock();
        let path = self.dir.join("config.json");
        if path.is_file() {
            write_json(&self.dir.join("backup/config-previous.json"), &*c)?;
        }
        write_json(&path, &config)?;
        *c = config;
        Ok(())
    }

    pub fn backups(&self) -> Result<Vec<Backup>, String> {
        task_backups(&self.dir)
    }

    pub fn sticky_position(&self) -> Option<StickyPosition> {
        read_json(&self.dir.join("sticky-position.json")).ok()
    }

    pub fn save_sticky_position(&self, x: i32, y: i32) -> Result<(), String> {
        write_json(
            &self.dir.join("sticky-position.json"),
            &StickyPosition { x, y },
        )
    }

    pub fn restore_backup(&self, id: &str) -> Result<(), String> {
        let backup = task_backup_path(&self.dir, id)?;
        let mut restored = read_task_backup(&backup)?;
        migrate_priorities(&mut restored);
        // Invalidate open editor versions while keeping reminder history intact.
        let version = crate::models::now_utc();
        for task in &mut restored.tasks {
            task.updated_at.clone_from(&version);
        }
        let mut data = self.data.lock();
        write_json(&snapshot_path(&self.dir, "restore"), &*data)?;
        write_json(&self.dir.join("data.json"), &restored)?;
        *data = restored;
        prune_backups(&self.dir, "restore-");
        Ok(())
    }

    pub fn mutate<R>(
        &self,
        change: impl FnOnce(&mut AppData) -> Result<R, String>,
    ) -> Result<R, String> {
        self.commit(false, change)
    }

    pub fn mutate_with_backup<R>(
        &self,
        change: impl FnOnce(&mut AppData) -> Result<R, String>,
    ) -> Result<R, String> {
        self.commit(true, change)
    }

    fn commit<R>(
        &self,
        backup: bool,
        change: impl FnOnce(&mut AppData) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut data = self.data.lock();
        let mut next = data.clone();
        let result = change(&mut next)?;
        if backup {
            write_json(&snapshot_path(&self.dir, "purge"), &*data)?;
        }
        write_json(&self.dir.join("data.json"), &next)?;
        *data = next;
        if backup {
            prune_backups(&self.dir, "purge-");
        }
        Ok(result)
    }
}

fn snapshot_path(dir: &Path, prefix: &str) -> PathBuf {
    dir.join("backup").join(format!(
        "{prefix}-{}-{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.9f"),
        uuid::Uuid::new_v4()
    ))
}

fn task_backup_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if ["daily-", "purge-", "restore-"]
        .iter()
        .any(|prefix| id.starts_with(prefix))
        && id.ends_with(".json")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
    {
        return Ok(dir.join("backup").join(id));
    }
    Err("备份名称无效".into())
}

fn read_task_backup(path: &Path) -> Result<AppData, String> {
    // A snapshot must contain a usable list. Missing files are errors here.
    let data: AppData = read_json(path)?;
    if data.tabs.is_empty() {
        return Err(file_error(path, "备份中没有任务列表"));
    }
    Ok(data)
}

fn task_backups(dir: &Path) -> Result<Vec<Backup>, String> {
    let folder = dir.join("backup");
    let mut snapshots: Vec<_> = fs::read_dir(&folder)
        .map_err(|err| file_error(&folder, err))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let id = entry.file_name().into_string().ok()?;
            let path = task_backup_path(dir, &id).ok()?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, id, path))
        })
        .collect();
    snapshots.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    Ok(snapshots
        .into_iter()
        .filter_map(|(modified, id, path)| {
            let data = read_task_backup(&path).ok()?;
            let trashed_tasks = data
                .tasks
                .iter()
                .filter(|task| task.deleted_at.is_some())
                .count();
            Some(Backup {
                id,
                saved_at: chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339(),
                active_tasks: data.tasks.len() - trashed_tasks,
                trashed_tasks,
            })
        })
        .collect())
}

pub fn startup_recovery(dir: &Path) -> Option<StartupRecovery> {
    let data = fs::read(dir.join("data.json"));
    if data.is_ok_and(|bytes| serde_json::from_slice::<AppData>(&bytes).is_err()) {
        return task_backups(dir)
            .ok()?
            .into_iter()
            .next()
            .map(StartupRecovery::Tasks);
    }
    let config = fs::read(dir.join("config.json"));
    config
        .is_ok_and(|bytes| serde_json::from_slice::<Config>(&bytes).is_err())
        .then(|| StartupRecovery::Settings {
            has_backup: read_json::<Config>(&dir.join("backup/config-previous.json")).is_ok(),
        })
}

pub fn recover_startup(dir: &Path, recovery: StartupRecovery) -> Result<(), String> {
    let (name, bytes) = match recovery {
        StartupRecovery::Tasks(backup) => {
            let path = task_backup_path(dir, &backup.id)?;
            let mut data = read_task_backup(&path)?;
            migrate_priorities(&mut data);
            ("data", serde_json::to_vec_pretty(&data))
        }
        StartupRecovery::Settings { has_backup } => {
            let config = if has_backup {
                read_json::<Config>(&dir.join("backup/config-previous.json"))?
            } else {
                Config::default()
            };
            ("config", serde_json::to_vec_pretty(&config))
        }
    };
    let original = dir.join(format!("{name}.json"));
    let bytes = bytes.map_err(|err| file_error(&original, err))?;
    let damaged = fs::read(&original).map_err(|err| file_error(&original, err))?;
    write_bytes(&snapshot_path(dir, &format!("damaged-{name}")), &damaged)?;
    write_bytes(&original, &bytes)
}

pub fn data_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("JUSTTODO_DATA_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(d) = dirs::data_dir() {
        let p = d.join("JustToDo");
        return p;
    }
    PathBuf::from("./JustToDo-data")
}

pub fn request_activation(dir: &Path) -> Result<(), String> {
    use io::Write;
    let path = dir.join(".show-sticky");
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(b"show\n").and_then(|()| file.sync_all()))
        .map_err(|err| file_error(&path, err))
}

pub fn take_activation(dir: &Path) -> bool {
    fs::remove_file(dir.join(".show-sticky")).is_ok()
}

fn load_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|err| file_error(path, err)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(file_error(path, err)),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|err| file_error(path, err))?;
    serde_json::from_slice(&bytes).map_err(|err| file_error(path, err))
}

fn migrate_priorities(data: &mut AppData) -> bool {
    if !data.tasks.iter().any(|task| task.priority == 5) {
        return false;
    }
    for task in &mut data.tasks {
        task.priority = if (1..=5).contains(&task.priority) {
            5 - task.priority
        } else {
            task.priority.clamp(0, 4)
        };
    }
    true
}

fn file_error(path: &Path, err: impl std::fmt::Display) -> String {
    format!("{}：{err}", path.display())
}

fn write_json<T: serde::Serialize>(path: &Path, val: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(val).map_err(|err| file_error(path, err))?;
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use io::Write;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(file_error(path, err));
    }
    Ok(())
}

fn prune_backups(dir: &Path, prefix: &str) {
    let Ok(entries) = fs::read_dir(dir.join("backup")) else {
        return;
    };
    let mut backups: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".json"))
        })
        .collect();
    backups.sort();
    for backup in backups.iter().take(backups.len().saturating_sub(14)) {
        let _ = fs::remove_file(backup);
    }
}

fn today_str() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%d").to_string()
}

// 单实例锁：文件句柄存活期间持有锁。
pub struct SingleInstance {
    _file: fs::File,
}

impl SingleInstance {
    pub fn acquire(dir: &Path) -> Result<Option<Self>, String> {
        fs::create_dir_all(dir).map_err(|err| file_error(dir, err))?;
        let path = dir.join(".lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|err| file_error(&path, err))?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(err) if err.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                Ok(None)
            }
            Err(err) => Err(file_error(&path, err)),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) struct TestDir(pub PathBuf);
    impl TestDir {
        pub fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("justtodo-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn repeated_writes_replace_json_and_restart_keeps_data() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        for index in 0..4 {
            store
                .mutate(|data| {
                    data.tabs[0].name = format!("list-{index}");
                    Ok(())
                })
                .unwrap();
        }
        drop(store);
        let restored = Store::open(dir.0.clone()).unwrap();
        assert_eq!(restored.data().tabs[0].name, "list-3");
        assert_eq!(fs::read_dir(dir.0.join("backup")).unwrap().count(), 1);
        assert!(fs::read_dir(&dir.0).unwrap().all(|entry| !entry
            .unwrap()
            .path()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn invalid_files_are_preserved_instead_of_resetting_tasks() {
        for name in ["data.json", "config.json"] {
            let dir = TestDir::new();
            fs::write(dir.0.join(name), b"{broken").unwrap();
            assert!(Store::open(dir.0.clone()).is_err());
            assert_eq!(fs::read(dir.0.join(name)).unwrap(), b"{broken");
        }
    }

    #[test]
    fn startup_recovery_keeps_corrupt_bytes_and_restores_the_latest_readable_snapshot() {
        let dir = TestDir::new();
        let backup_dir = dir.0.join("backup");
        fs::create_dir(&backup_dir).unwrap();
        let mut data = AppData::default();
        data.tabs[0].name = "from backup".into();
        write_json(&backup_dir.join("daily-2026-11-19.json"), &data).unwrap();
        fs::write(backup_dir.join("daily-2026-11-20.json"), b"invalid").unwrap();
        fs::write(dir.0.join("data.json"), b"{broken").unwrap();
        assert!(Store::open(dir.0.clone()).is_err());
        let recovery = startup_recovery(&dir.0).unwrap();
        assert!(matches!(&recovery, StartupRecovery::Tasks(b) if b.id == "daily-2026-11-19.json"));
        recover_startup(&dir.0, recovery).unwrap();
        let store = Store::open(dir.0.clone()).unwrap();
        assert_eq!(store.data().tabs[0].name, "from backup");
        let preserved = fs::read_dir(backup_dir)
            .unwrap()
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("damaged-data-")
            })
            .unwrap();
        assert_eq!(fs::read(preserved.path()).unwrap(), b"{broken");
    }

    #[test]
    fn restoring_a_snapshot_preserves_the_previous_state_and_rejects_invalid_paths() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        write_json(&dir.0.join("backup/daily-2026-11-19.json"), &store.data()).unwrap();
        store
            .mutate(|data| {
                data.tabs[0].name = "current".into();
                Ok(())
            })
            .unwrap();
        assert!(store.restore_backup("daily-../other.json").is_err());
        assert!(store.restore_backup("daily-missing.json").is_err());
        assert_eq!(store.data().tabs[0].name, "current");
        store.restore_backup("daily-2026-11-19.json").unwrap();
        assert_ne!(store.data().tabs[0].name, "current");
        let previous = store
            .backups()
            .unwrap()
            .into_iter()
            .find(|b| b.id.starts_with("restore-"))
            .unwrap();
        store.restore_backup(&previous.id).unwrap();
        assert_eq!(store.data().tabs[0].name, "current");
        let store = Store::open(dir.0.clone()).unwrap();
        assert_eq!(store.data().tabs[0].name, "current");
    }

    #[test]
    fn damaged_settings_can_be_restored_without_replacing_tasks() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        let mut config = store.config();
        config.window.opacity = 75;
        store.set_config(config.clone()).unwrap();
        config.window.opacity = 50;
        store.set_config(config).unwrap();
        store
            .mutate(|data| {
                data.tabs[0].name = "keep tasks".into();
                Ok(())
            })
            .unwrap();
        fs::write(dir.0.join("config.json"), b"broken").unwrap();
        let recovery = startup_recovery(&dir.0).unwrap();
        assert!(matches!(
            &recovery,
            StartupRecovery::Settings { has_backup: true }
        ));
        recover_startup(&dir.0, recovery).unwrap();
        let store = Store::open(dir.0.clone()).unwrap();
        assert_eq!(store.config().window.opacity, 75);
        assert_eq!(store.data().tabs[0].name, "keep tasks");
    }

    #[test]
    fn failed_commit_rolls_back_memory_and_config() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        let original = store.data().tabs[0].name.clone();
        assert!(store
            .mutate::<()>(|data| {
                data.tabs[0].name = "bad".into();
                Err("rejected".into())
            })
            .is_err());
        assert_eq!(store.data().tabs[0].name, original);
        fs::create_dir(dir.0.join("data.json")).unwrap();
        assert!(store
            .mutate(|data| {
                data.tabs[0].name = "bad".into();
                Ok(())
            })
            .is_err());
        assert_eq!(store.data().tabs[0].name, original);
        fs::create_dir(dir.0.join("config.json")).unwrap();
        let mut config = store.config();
        config.window.opacity = 45;
        assert!(store.set_config(config).is_err());
        assert_eq!(store.config().window.opacity, 100);
    }

    #[test]
    fn purge_backup_keeps_previous_state_and_rolls_to_fourteen() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        let original = store.data().tabs[0].name.clone();
        store
            .mutate_with_backup(|data| {
                data.tabs[0].name = "new".into();
                Ok(())
            })
            .unwrap();
        let path = fs::read_dir(dir.0.join("backup"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let backup: AppData = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(backup.tabs[0].name, original);
        for _ in 0..15 {
            store.mutate_with_backup(|_| Ok(())).unwrap();
        }
        assert_eq!(fs::read_dir(dir.0.join("backup")).unwrap().count(), 14);
    }

    #[test]
    fn backup_failure_blocks_destructive_change() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        fs::remove_dir(dir.0.join("backup")).unwrap();
        fs::write(dir.0.join("backup"), b"blocked").unwrap();
        assert!(store
            .mutate_with_backup(|data| {
                data.tabs.clear();
                Ok(())
            })
            .is_err());
        assert_eq!(store.data().tabs.len(), 3);
    }

    #[test]
    fn instance_lock_releases_when_owner_drops() {
        let dir = TestDir::new();
        let first = SingleInstance::acquire(&dir.0).unwrap().unwrap();
        assert!(SingleInstance::acquire(&dir.0).unwrap().is_none());
        assert!(!take_activation(&dir.0));
        request_activation(&dir.0).unwrap();
        request_activation(&dir.0).unwrap();
        assert!(take_activation(&dir.0));
        assert!(!take_activation(&dir.0));
        drop(first);
        assert!(SingleInstance::acquire(&dir.0).unwrap().is_some());
    }

    #[test]
    fn sticky_position_is_saved_and_loaded_without_touching_task_data() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        let before = store.data();
        store.save_sticky_position(-1920, 140).unwrap();
        let position = store.sticky_position().unwrap();
        assert_eq!((position.x, position.y), (-1920, 140));
        assert_eq!(store.data().tabs[0].name, before.tabs[0].name);
    }
}
