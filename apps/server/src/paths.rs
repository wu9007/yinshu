use std::path::PathBuf;

use yinshu_runtime::RuntimePaths;

/// 返回 headless 系统服务的固定配置、状态和运行目录。
pub fn system_paths() -> RuntimePaths {
    RuntimePaths::new(
        env_path("YINSHU_CONFIG_PATH", "/etc/yinshu/config.json"),
        env_path("YINSHU_DATA_DIR", "/var/lib/yinshu"),
        env_path("YINSHU_RUNTIME_DIR", "/run/yinshu"),
    )
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| default.into())
}
