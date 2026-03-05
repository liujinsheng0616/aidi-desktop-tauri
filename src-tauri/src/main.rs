// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 设置 panic hook 捕获崩溃信息并写入文件
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let full_msg = format!(
            "=== AIDI 崩溃报告 ===\n时间: {}\n错误: {}\n位置: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            msg,
            location
        );
        eprintln!("{}", full_msg);

        // 尝试写入崩溃日志到多个位置
        let crash_locations: Vec<Option<std::path::PathBuf>> = vec![
            dirs::desktop_dir().map(|p| p.join("aidi-crash.log")),
            dirs::data_local_dir().map(|p| {
                let dir = p.join("aidi-desktop");
                let _ = std::fs::create_dir_all(&dir);
                dir.join("crash.log")
            }),
            std::env::current_exe().ok().and_then(|exe| {
                exe.parent().map(|p| p.join("aidi-crash.log"))
            }),
        ];

        for location in crash_locations.into_iter().flatten() {
            if std::fs::write(&location, &full_msg).is_ok() {
                eprintln!("崩溃日志已写入: {:?}", location);
                return;
            }
        }
        eprintln!("无法写入崩溃日志文件");
    }));

    aidi_desktop_tauri_lib::run()
}
