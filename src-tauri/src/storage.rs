// JustToDo 存储层：Rust 侧单一数据源
// - data.json / config.json 持久化
// - Parking_lot Mutex 内存缓存
// - 每日备份 + 删除前备份
// - 单实例文件锁

use crate::models::{AppData, Config, APP_VERSION};
use parking_lot::Mutex;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Store {
    data: Mutex<AppData>,
    config: Mutex<Config>,
    dir: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let dir = data_dir();
        fs::create_dir_all(&dir).ok();
        fs::create_dir_all(dir.join("backup")).ok();

        let mut data = load_or_default(&dir.join("data.json"), AppData::default());
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
            write_json(&dir.join("data.json"), &data).ok();
        }
        let config_path = dir.join("config.json");
        let mut config = load_or_default(&config_path, Config::default());
        if config.version != APP_VERSION {
            config.version = APP_VERSION.into();
            write_json(&config_path, &config).ok();
        }

        Store {
            data: Mutex::new(data),
            config: Mutex::new(config),
            dir,
        }
    }

    pub fn data(&self) -> AppData {
        self.data.lock().clone()
    }

    pub fn config(&self) -> Config {
        self.config.lock().clone()
    }

    pub fn set_config(&self, config: Config) {
        let mut c = self.config.lock();
        *c = config;
        drop(c);
        self.save_config();
    }

    pub fn with_data_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut AppData) -> R,
    {
        // 注意：parking_lot::Mutex 非可重入，必须先释放锁再落盘，
        // 否则 save_data() 再次加锁会死锁（启动时 purge_expired_trash 即触发此路径）。
        let r = {
            let mut data = self.data.lock();
            f(&mut data)
        };
        self.save_data();
        r
    }

    fn save_data(&self) {
        let data = self.data.lock();
        let path = self.dir.join("data.json");
        write_json(&path, &*data).ok();
    }

    fn save_config(&self) {
        let cfg = self.config.lock();
        let path = self.dir.join("config.json");
        write_json(&path, &*cfg).ok();
    }

    pub fn backup_before_purge(&self) {
        let data = self.data.lock();
        let name = format!("trash-{}.json", today_str());
        let path = self.dir.join("backup").join(name);
        write_json(&path, &*data).ok();
    }
}

fn data_dir() -> PathBuf {
    if let Some(d) = dirs::data_dir() {
        let p = d.join("JustToDo");
        return p;
    }
    PathBuf::from("./JustToDo-data")
}

fn load_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path, default: T) -> T {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or(default),
        Err(_) => default,
    }
}

fn write_json<T: serde::Serialize>(path: &Path, val: &T) -> Result<(), serde_json::Error> {
    let s = serde_json::to_string_pretty(val)?;
    fs::write(path, s).ok();
    Ok(())
}

fn today_str() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%d").to_string()
}

// 单实例锁：运行时占用 .lock，已存在则表示有实例
pub struct SingleInstance {
    path: PathBuf,
}

impl SingleInstance {
    pub fn acquire() -> Option<Self> {
        let path = data_dir().join(".lock");
        fs::create_dir_all(path.parent().unwrap()).ok();
        use fs2::FileExt;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&path)
            .ok()?;
        if file.try_lock_exclusive().is_err() {
            return None;
        }
        std::mem::forget(file); // 保持锁到进程结束
        Some(SingleInstance { path })
    }
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
