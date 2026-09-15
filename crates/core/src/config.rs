use crate::{
    ip_whitelist::{normalize_allowed_ips, REQUIRED_LOOPBACK_IP},
    protocol::EffectivePaper,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// 系统配置目录中使用的应用目录名。
pub const APP_CONFIG_DIR_NAME: &str = "cn.yinshu.app";
/// Agent 配置文件名。
pub const CONFIG_FILE_NAME: &str = "config.json";
/// 本地任务历史数据库文件名。
pub const TASK_HISTORY_FILE_NAME: &str = "task_history.sqlite3";
/// 覆盖配置文件路径的环境变量名。
pub const CONFIG_PATH_OVERRIDE_ENV: &str = "YINSHU_CONFIG_PATH";
/// 覆盖数据目录路径的环境变量名。
pub const DATA_DIR_OVERRIDE_ENV: &str = "YINSHU_DATA_DIR";
/// 覆盖日志目录路径的环境变量名。
pub const LOG_DIR_OVERRIDE_ENV: &str = "YINSHU_LOG_DIR";
/// 覆盖诊断包输出目录的环境变量名。
pub const DIAGNOSE_DIR_OVERRIDE_ENV: &str = "YINSHU_DIAGNOSE_DIR";
/// 启动失败日志文件名。
pub const STARTUP_ERROR_FILE_NAME: &str = "yinshu-startup-error.txt";
/// 安装程序日志文件名。
pub const INSTALL_LOG_FILE_NAME: &str = "install.log";

/// 用户可配置的最小本地服务端口。
pub const MIN_SERVICE_PORT: u16 = 10000;
/// 用户可配置的最大本地服务端口。
pub const MAX_SERVICE_PORT: u16 = u16::MAX;
/// YinShu Agent 默认本地服务端口。
pub const DEFAULT_PORT: u16 = 17890;
/// 返回默认 IP 白名单，保留本机回环地址。
fn default_allowed_ips() -> Vec<String> {
    vec![REQUIRED_LOOPBACK_IP.to_string()]
}

fn default_ui_language() -> UiLanguage {
    UiLanguage::ZhCn
}

/// 返回 CLI 使用的配置文件路径。
pub fn cli_config_path() -> Result<PathBuf, io::Error> {
    if let Some(path) = env::var_os(CONFIG_PATH_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    Ok(cli_data_dir()?.join(CONFIG_FILE_NAME))
}

/// 返回 CLI 使用的任务历史数据库路径。
pub fn cli_task_history_path() -> Result<PathBuf, io::Error> {
    Ok(cli_data_dir()?.join(TASK_HISTORY_FILE_NAME))
}

/// 返回 CLI 和 headless serve 使用的数据目录。
pub fn cli_data_dir() -> Result<PathBuf, io::Error> {
    if let Some(dir) = env::var_os(DATA_DIR_OVERRIDE_ENV) {
        return Ok(PathBuf::from(dir));
    }

    platform_config_dir().map(|dir| dir.join(APP_CONFIG_DIR_NAME))
}

/// 返回与 Tauri 日志插件一致的日志目录。
pub fn cli_log_dir() -> Result<PathBuf, io::Error> {
    if let Some(dir) = env::var_os(LOG_DIR_OVERRIDE_ENV) {
        return Ok(PathBuf::from(dir));
    }

    platform_log_dir()
}

/// 返回启动失败日志路径。
pub fn startup_error_log_path() -> Result<PathBuf, io::Error> {
    Ok(cli_log_dir()?.join(STARTUP_ERROR_FILE_NAME))
}

/// 返回桌面上的默认诊断包路径。
pub fn default_diagnose_output_path() -> Result<PathBuf, io::Error> {
    if let Some(dir) = env::var_os(DIAGNOSE_DIR_OVERRIDE_ENV) {
        return Ok(PathBuf::from(dir).join("yinshu-diagnose.zip"));
    }

    Ok(platform_desktop_dir()?.join("yinshu-diagnose.zip"))
}

#[cfg(target_os = "windows")]
fn platform_config_dir() -> Result<PathBuf, io::Error> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))
}

#[cfg(target_os = "macos")]
fn platform_config_dir() -> Result<PathBuf, io::Error> {
    home_dir().map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_config_dir() -> Result<PathBuf, io::Error> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(dir));
    }

    home_dir().map(|home| home.join(".config"))
}

#[cfg(unix)]
fn home_dir() -> Result<PathBuf, io::Error> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

#[cfg(windows)]
fn home_dir() -> Result<PathBuf, io::Error> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "USERPROFILE is not set"))
}

#[cfg(target_os = "windows")]
fn platform_log_dir() -> Result<PathBuf, io::Error> {
    env::var_os("LOCALAPPDATA")
        .map(|dir| PathBuf::from(dir).join(APP_CONFIG_DIR_NAME).join("logs"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))
}

#[cfg(target_os = "macos")]
fn platform_log_dir() -> Result<PathBuf, io::Error> {
    home_dir().map(|home| home.join("Library").join("Logs").join(APP_CONFIG_DIR_NAME))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_log_dir() -> Result<PathBuf, io::Error> {
    if let Some(dir) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(dir).join(APP_CONFIG_DIR_NAME).join("logs"));
    }

    home_dir().map(|home| {
        home.join(".local")
            .join("share")
            .join(APP_CONFIG_DIR_NAME)
            .join("logs")
    })
}

fn platform_desktop_dir() -> Result<PathBuf, io::Error> {
    home_dir().map(|home| home.join("Desktop"))
}

/// 本地 YinShu Agent 的持久化配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub service: ServiceConfig,
    pub security: SecurityConfig,
    pub printing: PrintingConfig,
    pub limits: LimitsConfig,
    pub app: AppConfig,
}

/// 本地 HTTP/WebSocket 服务绑定设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub host: String,
    pub port: u16,
}

/// 允许打开 YinShu WebSocket 会话的浏览器 Origin。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub allowed_origins: Vec<String>,
    #[serde(default = "default_allowed_ips")]
    pub allowed_ips: Vec<String>,
}

/// 打印任务未提供覆盖项时使用的默认打印设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintingConfig {
    pub default_printer: Option<String>,
    pub default_paper: Option<EffectivePaper>,
    pub default_copies: u16,
}

/// 下载、批量任务和打印份数的运行时安全限制。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_file_size_mb: u64,
    pub max_batch_jobs: usize,
    pub max_copies: u16,
    pub download_timeout_seconds: u64,
}

/// 桌面应用偏好设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub autostart: bool,
    #[serde(default = "default_ui_language")]
    pub language: UiLanguage,
}

/// 桌面 UI 语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiLanguage {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

impl Default for AgentConfig {
    /// 创建首次运行配置，并使用仅限本机访问的服务默认值。
    fn default() -> Self {
        Self {
            service: ServiceConfig {
                host: "127.0.0.1".to_string(),
                port: DEFAULT_PORT,
            },
            security: SecurityConfig {
                allowed_origins: Vec::new(),
                allowed_ips: default_allowed_ips(),
            },
            printing: PrintingConfig {
                default_printer: None,
                default_paper: None,
                default_copies: 1,
            },
            limits: LimitsConfig {
                max_file_size_mb: 20,
                max_batch_jobs: 20,
                max_copies: 100,
                download_timeout_seconds: 30,
            },
            app: AppConfig {
                autostart: false,
                language: UiLanguage::ZhCn,
            },
        }
    }
}

impl AgentConfig {
    /// 保持兼容字段为本机默认值；服务端实际监听地址由 server 模块决定。
    pub fn normalized(mut self) -> Self {
        self.service.host = "127.0.0.1".to_string();
        self.security.allowed_ips = normalize_allowed_ips(self.security.allowed_ips);
        self
    }

    /// 从磁盘加载配置；文件不存在时返回默认配置。
    pub fn load(path: &Path) -> Result<Self, io::Error> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        serde_json::from_str::<Self>(&content)
            .map(Self::normalized)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    /// 把规范化后的配置保存到磁盘，必要时创建父目录。
    pub fn save(&self, path: &Path) -> Result<(), io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config = self.clone().normalized();
        let content =
            serde_json::to_string_pretty(&config).expect("AgentConfig should always serialize");
        fs::write(path, content)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{
        env,
        ffi::OsString,
        sync::{Mutex, OnceLock},
    };
    use uuid::Uuid;

    pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    pub(crate) struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe {
                    env::set_var(self.key, value);
                },
                None => unsafe {
                    env::remove_var(self.key);
                },
            }
        }
    }

    pub(crate) fn set_env(key: &'static str, value: &std::path::Path) -> EnvGuard {
        let previous = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        EnvGuard { key, previous }
    }

    fn clear_env(key: &'static str) -> EnvGuard {
        let previous = env::var_os(key);
        unsafe {
            env::remove_var(key);
        }
        EnvGuard { key, previous }
    }

    #[test]
    fn cli_data_dir_uses_data_dir_override() {
        let _lock = test_lock();
        let data_dir = std::env::temp_dir().join(format!("yinshu-data-{}", Uuid::new_v4()));
        let _data_guard = set_env(DATA_DIR_OVERRIDE_ENV, &data_dir);
        let _config_guard = clear_env(CONFIG_PATH_OVERRIDE_ENV);

        assert_eq!(cli_data_dir().unwrap(), data_dir);
    }

    #[test]
    fn cli_config_path_prefers_config_path_override() {
        let _lock = test_lock();
        let data_dir = std::env::temp_dir().join(format!("yinshu-data-{}", Uuid::new_v4()));
        let config_path = std::env::temp_dir().join(format!("yinshu-{}.json", Uuid::new_v4()));
        let _data_guard = set_env(DATA_DIR_OVERRIDE_ENV, &data_dir);
        let _config_guard = set_env(CONFIG_PATH_OVERRIDE_ENV, &config_path);

        assert_eq!(cli_config_path().unwrap(), config_path);
        assert_eq!(
            cli_task_history_path().unwrap(),
            data_dir.join(TASK_HISTORY_FILE_NAME)
        );
    }

    #[test]
    fn cli_log_dir_uses_log_dir_override() {
        let _lock = test_lock();
        let log_dir = std::env::temp_dir().join(format!("yinshu-logs-{}", Uuid::new_v4()));
        let _guard = set_env(LOG_DIR_OVERRIDE_ENV, &log_dir);

        assert_eq!(cli_log_dir().unwrap(), log_dir);
        assert_eq!(
            startup_error_log_path().unwrap(),
            log_dir.join(STARTUP_ERROR_FILE_NAME)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_log_dir_matches_tauri_localappdata() {
        let _lock = test_lock();
        let local = std::env::temp_dir().join(format!("yinshu-local-{}", Uuid::new_v4()));
        let _log_guard = clear_env(LOG_DIR_OVERRIDE_ENV);
        let _local_guard = set_env("LOCALAPPDATA", &local);

        assert_eq!(
            cli_log_dir().unwrap(),
            local.join(APP_CONFIG_DIR_NAME).join("logs")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_log_dir_matches_tauri_library_logs() {
        let _lock = test_lock();
        let home = std::env::temp_dir().join(format!("yinshu-home-{}", Uuid::new_v4()));
        let _log_guard = clear_env(LOG_DIR_OVERRIDE_ENV);
        let _home_guard = set_env("HOME", &home);

        assert_eq!(
            cli_log_dir().unwrap(),
            home.join("Library").join("Logs").join(APP_CONFIG_DIR_NAME)
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_log_dir_matches_tauri_xdg_data() {
        let _lock = test_lock();
        let data = std::env::temp_dir().join(format!("yinshu-xdg-{}", Uuid::new_v4()));
        let _log_guard = clear_env(LOG_DIR_OVERRIDE_ENV);
        let _xdg_guard = set_env("XDG_DATA_HOME", &data);

        assert_eq!(
            cli_log_dir().unwrap(),
            data.join(APP_CONFIG_DIR_NAME).join("logs")
        );
    }

    #[test]
    fn diagnose_output_uses_override_dir() {
        let _lock = test_lock();
        let dir = std::env::temp_dir().join(format!("yinshu-diagnose-{}", Uuid::new_v4()));
        let _guard = set_env(DIAGNOSE_DIR_OVERRIDE_ENV, &dir);

        assert_eq!(
            default_diagnose_output_path().unwrap(),
            dir.join("yinshu-diagnose.zip")
        );
    }
}
