use yinshu_lib::config::{AgentConfig, CONFIG_FILE_NAME, TASK_HISTORY_FILE_NAME};

#[test]
fn persisted_file_names_and_default_port_are_stable() {
    assert_eq!(CONFIG_FILE_NAME, "config.json");
    assert_eq!(TASK_HISTORY_FILE_NAME, "task_history.sqlite3");
    assert_eq!(AgentConfig::default().service.port, 17890);
}
