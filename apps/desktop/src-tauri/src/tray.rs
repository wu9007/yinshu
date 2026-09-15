use crate::{config::UiLanguage, state::AgentState, test_print::print_test_page};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

struct TrayLabels {
    open_settings: &'static str,
    test_print: &'static str,
    quit: &'static str,
}

struct TrayMenuItems {
    open_settings: MenuItem<tauri::Wry>,
    test_print: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// 创建系统托盘菜单，并把菜单动作接到应用状态。
pub fn setup_tray(app: &mut App, language: UiLanguage) -> tauri::Result<()> {
    let labels = tray_labels(language);
    let open = MenuItem::with_id(
        app,
        "open_settings",
        labels.open_settings,
        true,
        None::<&str>,
    )?;
    let test = MenuItem::with_id(app, "test_print", labels.test_print, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &test, &quit])?;

    app.manage(TrayMenuItems {
        open_settings: open.clone(),
        test_print: test.clone(),
        quit: quit.clone(),
    });

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(Image::from_bytes(TRAY_ICON_BYTES)?);

    tray.show_menu_on_left_click(shows_menu_on_left_click())
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_settings" => show_main_window(app),
            "test_print" => test_print(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// 按当前界面语言更新已创建的托盘菜单文本。
pub fn apply_tray_language(app: &tauri::AppHandle, language: UiLanguage) -> tauri::Result<()> {
    let Some(items) = app.try_state::<TrayMenuItems>() else {
        return Ok(());
    };
    let labels = tray_labels(language);

    items.open_settings.set_text(labels.open_settings)?;
    items.test_print.set_text(labels.test_print)?;
    items.quit.set_text(labels.quit)?;

    Ok(())
}

fn tray_labels(language: UiLanguage) -> TrayLabels {
    match language {
        UiLanguage::ZhCn => TrayLabels {
            open_settings: "打开设置",
            test_print: "测试打印",
            quit: "退出",
        },
        UiLanguage::En => TrayLabels {
            open_settings: "Open Settings",
            test_print: "Test Print",
            quit: "Quit",
        },
    }
}

/// 左键打开窗口，右键才弹出菜单。
fn shows_menu_on_left_click() -> bool {
    false
}

/// 使用当前默认打印设置提交一张配置测试页。
fn test_print(app: &tauri::AppHandle) {
    let Some(state) = app
        .try_state::<AgentState>()
        .map(|state| state.inner().clone())
    else {
        tauri_plugin_log::log::error!("failed to run test print: app state is not initialized");
        return;
    };

    tauri::async_runtime::spawn(async move {
        if let Err(error) = print_test_page(&state).await {
            tauri_plugin_log::log::error!("test print failed: {error}");
        }
    });
}

/// 如果主设置窗口存在，则显示并聚焦它。
pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_dock_visibility(true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::tray_labels;
    use crate::config::UiLanguage;

    #[test]
    fn tray_labels_returns_chinese_labels() {
        let labels = tray_labels(UiLanguage::ZhCn);

        assert_eq!(labels.open_settings, "打开设置");
        assert_eq!(labels.test_print, "测试打印");
        assert_eq!(labels.quit, "退出");
    }

    #[test]
    fn tray_labels_returns_english_labels() {
        let labels = tray_labels(UiLanguage::En);

        assert_eq!(labels.open_settings, "Open Settings");
        assert_eq!(labels.test_print, "Test Print");
        assert_eq!(labels.quit, "Quit");
    }

    #[test]
    fn left_click_opens_window_without_menu() {
        assert!(!super::shows_menu_on_left_click());
    }
}
