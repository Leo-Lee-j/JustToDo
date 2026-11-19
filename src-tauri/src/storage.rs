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
        if data.tasks.iter().any(|task| task.priority == 5) {
            for task in &mut data.tasks {
                if (1..=5).contains(&task.priority) {
                    task.priority = 5 - task.priority;
                } else {
                    task.priority = task.priority.clamp(0, 4);
                }
            }
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
        write_json(&self.dir.join("config.json"), &config)?;
        *c = config;
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
            let name = format!(
                "purge-{}-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%S%.9f"),
                uuid::Uuid::new_v4()
            );
            write_json(&self.dir.join("backup").join(name), &*data)?;
        }
        write_json(&self.dir.join("data.json"), &next)?;
        *data = next;
        if backup {
            prune_backups(&self.dir, "purge-");
        }
        Ok(result)
    }
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

fn load_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|err| file_error(path, err)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(file_error(path, err)),
    }
}

fn file_error(path: &Path, err: impl std::fmt::Display) -> String {
    format!("{}：{err}", path.display())
}

fn write_json<T: serde::Serialize>(path: &Path, val: &T) -> Result<(), String> {
    use io::Write;
    let bytes = serde_json::to_vec_pretty(val).map_err(|err| file_error(path, err))?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
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
        drop(first);
        assert!(SingleInstance::acquire(&dir.0).unwrap().is_some());
    }
}
