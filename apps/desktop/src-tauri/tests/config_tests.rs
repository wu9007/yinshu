use std::fs;
use std::sync::Mutex;
use yinshu_lib::{
    config::{
        cli_config_path, cli_task_history_path, AgentConfig, AppConfig, LimitsConfig,
        PrintingConfig, SecurityConfig, ServiceConfig, UiLanguage, CONFIG_PATH_OVERRIDE_ENV,
        DATA_DIR_OVERRIDE_ENV,
    },
    protocol::EffectivePaper,
};

static ENV_TEST_MUTEX: Mutex<()> = Mutex::new(());

struct EnvVarOverrideGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarOverrideGuard {
    fn set_env_var<K: AsRef<std::ffi::OsStr>>(key: &'static str, value: K) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarOverrideGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

#[test]
fn agent_config_defaults_match_grouped_baseline() {
    let config = AgentConfig::default();

    assert_eq!(config.service.host, "127.0.0.1");
    assert_eq!(config.service.port, 17890);
    assert!(config.security.allowed_origins.is_empty());
    assert_eq!(config.security.allowed_ips, vec!["127.0.0.1".to_string()]);
    assert_eq!(config.printing.default_printer, None);
    assert_eq!(config.printing.default_paper, None);
    assert_eq!(config.printing.default_copies, 1);
    assert_eq!(config.limits.max_file_size_mb, 20);
    assert_eq!(config.limits.max_batch_jobs, 20);
    assert_eq!(config.limits.max_copies, 100);
    assert_eq!(config.limits.download_timeout_seconds, 30);
    assert!(!config.app.autostart);
    assert_eq!(config.app.language, UiLanguage::ZhCn);
}

#[test]
fn agent_config_json_roundtrips() {
    let config = AgentConfig {
        service: ServiceConfig {
            host: "0.0.0.0".to_string(),
            port: 19000,
        },
        security: SecurityConfig {
            allowed_origins: vec!["http://localhost:5173".to_string()],
            allowed_ips: vec!["127.0.0.1".to_string(), "192.168.1.0/24".to_string()],
        },
        printing: PrintingConfig {
            default_printer: Some("TSC TE244".to_string()),
            default_paper: Some(EffectivePaper {
                width_mm: 60.0,
                height_mm: 40.0,
            }),
            default_copies: 2,
        },
        limits: LimitsConfig {
            max_file_size_mb: 10,
            max_batch_jobs: 3,
            max_copies: 8,
            download_timeout_seconds: 15,
        },
        app: AppConfig {
            autostart: true,
            language: UiLanguage::En,
        },
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: AgentConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, config);
}

#[test]
fn agent_config_ignores_legacy_remote_json() {
    let json = r#"{
        "service": { "host": "127.0.0.1", "port": 17890 },
        "security": { "allowed_origins": [] },
        "printing": {
            "default_printer": null,
            "default_paper": null,
            "default_copies": 1
        },
        "limits": {
            "max_file_size_mb": 20,
            "max_batch_jobs": 20,
            "max_copies": 100,
            "download_timeout_seconds": 30
        },
        "app": { "autostart": false },
        "remote": {
            "enabled": true,
            "endpoint_url": "https://legacy.example.com/tasks"
        }
    }"#;

    let decoded: AgentConfig = serde_json::from_str(json).unwrap();

    assert_eq!(decoded.service.port, 17890);
    assert_eq!(decoded.security.allowed_ips, vec!["127.0.0.1".to_string()]);
    assert_eq!(decoded.app.language, UiLanguage::ZhCn);
}

#[test]
fn agent_config_loads_legacy_json_without_allowed_ips() {
    let json = r#"{
        "service": { "host": "127.0.0.1", "port": 17890 },
        "security": { "allowed_origins": ["https://example.com"] },
        "printing": {
            "default_printer": null,
            "default_paper": null,
            "default_copies": 1
        },
        "limits": {
            "max_file_size_mb": 20,
            "max_batch_jobs": 20,
            "max_copies": 100,
            "download_timeout_seconds": 30
        },
        "app": { "autostart": false }
    }"#;

    let decoded: AgentConfig = serde_json::from_str(json).unwrap();

    assert_eq!(
        decoded.security.allowed_origins,
        vec!["https://example.com"]
    );
    assert_eq!(decoded.security.allowed_ips, vec!["127.0.0.1"]);
    assert_eq!(decoded.app.language, UiLanguage::ZhCn);
}

#[test]
fn agent_config_load_returns_default_when_file_is_missing() {
    let path =
        std::env::temp_dir().join(format!("yinshu-missing-config-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);

    let config = AgentConfig::load(&path).unwrap();

    assert_eq!(config, AgentConfig::default());
}

#[test]
fn agent_config_save_and_load_roundtrips() {
    let path = std::env::temp_dir().join(format!(
        "yinshu-config-roundtrip-{}.json",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    let config = AgentConfig::default();

    config.save(&path).unwrap();
    let loaded = AgentConfig::load(&path).unwrap();

    assert_eq!(loaded, config);

    let _ = fs::remove_file(&path);
}

#[test]
fn agent_config_load_returns_error_for_invalid_json() {
    let path =
        std::env::temp_dir().join(format!("yinshu-invalid-config-{}.json", std::process::id()));
    fs::write(&path, "{ invalid json").unwrap();

    let result = AgentConfig::load(&path);

    assert!(result.is_err());

    let _ = fs::remove_file(&path);
}

#[test]
fn cli_config_path_uses_explicit_file_override() {
    let _env_lock = ENV_TEST_MUTEX.lock().unwrap();

    let path = std::env::temp_dir().join(format!(
        "yinshu-cli-config-path-{}.json",
        std::process::id()
    ));
    let _override_guard = EnvVarOverrideGuard::set_env_var(CONFIG_PATH_OVERRIDE_ENV, &path);

    let resolved = cli_config_path().unwrap();
    assert_eq!(resolved, path);
}

#[test]
fn cli_task_history_path_uses_data_dir_override() {
    let _env_lock = ENV_TEST_MUTEX.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("yinshu-cli-data-dir-{}", std::process::id()));
    let expected = dir.join("task_history.sqlite3");
    let _override_guard = EnvVarOverrideGuard::set_env_var(DATA_DIR_OVERRIDE_ENV, &dir);

    let resolved = cli_task_history_path().unwrap();
    assert_eq!(resolved, expected);
}
