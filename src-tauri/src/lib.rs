use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IconRule {
    id: String,
    name: String,
    target: String,
    kind: String,
    icon_path: String,
    enabled: bool,
    #[serde(default = "default_true")]
    monitored: bool,
    last_applied: Option<u64>,
    status: Option<String>,
    backup: Option<String>,
    #[serde(default = "default_recursive_mode")]
    recursive_mode: String,
    #[serde(default)]
    max_depth: Option<usize>,
    #[serde(default)]
    applied_targets: Vec<AppliedTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppliedTarget {
    target: String,
    backup: Option<String>,
}

#[cfg(target_os = "windows")]
#[derive(Serialize, Deserialize)]
struct WindowsExtensionBackup {
    version: u8,
    old_direct_icon: Option<String>,
    prog_id: Option<String>,
    old_prog_icon: Option<String>,
}
fn default_recursive_mode() -> String {
    "none".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    monitor_enabled: bool,
    monitor_interval_minutes: u64,
    autostart: bool,
    rules: Vec<IconRule>,
    #[serde(default)]
    easter_batches: Vec<EasterBatch>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            monitor_enabled: false,
            monitor_interval_minutes: 10,
            autostart: false,
            rules: vec![],
            easter_batches: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EasterBatch {
    id: String,
    root: String,
    icon_path: String,
    created_at: u64,
    directory_count: usize,
    file_count: usize,
    extensions: Vec<String>,
    items: Vec<EasterItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EasterItem {
    target: String,
    kind: String,
    backup: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EasterPreview {
    directory_count: usize,
    file_count: usize,
    extensions: Vec<String>,
}

fn data_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("无法确定应用数据目录")?
        .join("IconCraft");
    fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败：{e}"))?;
    Ok(dir)
}
fn settings_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("settings.json"))
}
fn read_settings() -> AppSettings {
    settings_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn write_settings(settings: &AppSettings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(settings_path()?, json).map_err(|e| format!("保存设置失败：{e}"))
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[tauri::command]
fn load_settings() -> AppSettings {
    read_settings()
}

#[tauri::command]
fn save_settings(settings: AppSettings) -> Result<(), String> {
    write_settings(&settings)
}

#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "other"
    }
}

fn collect_directories(
    root: &Path,
    mode: &str,
    max_depth: Option<usize>,
) -> Result<Vec<PathBuf>, String> {
    let limit = match mode {
        "none" => 0,
        "depth" => max_depth.unwrap_or(1).clamp(1, 10),
        "all" => usize::MAX,
        _ => return Err("递归模式无效".into()),
    };
    let mut result = vec![root.to_path_buf()];
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth >= limit {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.is_symlink() {
                result.push(path.clone());
                stack.push((path, depth + 1));
            }
        }
    }
    Ok(result)
}

fn collect_easter(
    root: &Path,
    mode: &str,
    max_depth: Option<usize>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<String>), String> {
    if !root.is_dir() {
        return Err("彩蛋模式根目录不存在".into());
    }
    let limit = match mode {
        "none" => 0,
        "depth" => max_depth.unwrap_or(1).clamp(1, 10),
        "all" => usize::MAX,
        _ => return Err("递归模式无效".into()),
    };
    let mut dirs = vec![root.to_path_buf()];
    let mut files = vec![];
    let mut extensions = std::collections::BTreeSet::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_symlink() {
                continue;
            }
            if p.is_dir() {
                if depth < limit {
                    dirs.push(p.clone());
                    stack.push((p, depth + 1));
                }
            } else if p.is_file() {
                files.push(p.clone());
                if let Some(ext) = p.extension().and_then(|v| v.to_str()) {
                    if !ext.is_empty() {
                        extensions.insert(format!(".{}", ext.to_lowercase()));
                    }
                }
            }
        }
    }
    Ok((dirs, files, extensions.into_iter().collect()))
}

#[tauri::command]
fn preview_easter(
    root: String,
    recursive_mode: String,
    max_depth: Option<usize>,
) -> Result<EasterPreview, String> {
    let (dirs, files, extensions) = collect_easter(Path::new(&root), &recursive_mode, max_depth)?;
    Ok(EasterPreview {
        directory_count: dirs.len(),
        file_count: files.len(),
        extensions,
    })
}

#[tauri::command]
fn apply_easter(
    root: String,
    recursive_mode: String,
    max_depth: Option<usize>,
    png_data: String,
) -> Result<AppSettings, String> {
    let (dirs, files, extensions) = collect_easter(Path::new(&root), &recursive_mode, max_depth)?;
    let id = uuid::Uuid::new_v4().to_string();
    let icon_dir = data_dir()?.join("icons");
    fs::create_dir_all(&icon_dir).map_err(|e| e.to_string())?;
    let png = icon_dir.join(format!("easter-{id}.png"));
    fs::write(
        &png,
        STANDARD
            .decode(png_data)
            .map_err(|e| format!("图片数据无效：{e}"))?,
    )
    .map_err(|e| e.to_string())?;
    let icon = prepare_native_icon(&png, &format!("easter-{id}"))?;
    let mut items = vec![];
    let mut apply_one = |target: String, kind: &str| -> Result<(), String> {
        match apply_native(&target, kind, &icon, true) {
            Ok(backup) => {
                items.push(EasterItem {
                    target,
                    kind: kind.into(),
                    backup,
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    };
    let result = (|| {
        for dir in &dirs {
            apply_one(dir.to_string_lossy().into_owned(), "folder")?
        }
        #[cfg(target_os = "macos")]
        for file in &files {
            apply_one(file.to_string_lossy().into_owned(), "file")?
        }
        #[cfg(target_os = "windows")]
        for ext in &extensions {
            apply_one(ext.clone(), "extension")?
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = result {
        for item in items.iter().rev() {
            let _ = restore_native(&item.target, &item.kind, item.backup.as_deref());
        }
        let _ = fs::remove_file(&icon);
        return Err(format!("批量应用失败并已回滚：{error}"));
    }
    let batch = EasterBatch {
        id,
        root,
        icon_path: icon.to_string_lossy().into_owned(),
        created_at: now(),
        directory_count: dirs.len(),
        file_count: files.len(),
        extensions,
        items,
    };
    let mut settings = read_settings();
    settings.easter_batches.push(batch);
    write_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn restore_easter(id: String) -> Result<AppSettings, String> {
    let mut settings = read_settings();
    let batch = settings
        .easter_batches
        .iter()
        .find(|v| v.id == id)
        .cloned()
        .ok_or("彩蛋批次不存在")?;
    let mut errors = vec![];
    for item in batch.items.iter().rev() {
        if let Err(e) = restore_native(&item.target, &item.kind, item.backup.as_deref()) {
            errors.push(e)
        }
    }
    if !errors.is_empty() {
        return Err(format!("部分目标还原失败：{}", errors.join("；")));
    }
    settings.easter_batches.retain(|v| v.id != id);
    write_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn apply_icon(
    target: String,
    kind: String,
    png_data: String,
    name: String,
    add_to_monitor: bool,
    recursive_mode: String,
    max_depth: Option<usize>,
) -> Result<IconRule, String> {
    validate_target(&target, &kind)?;
    let mut settings = read_settings();
    let existing = settings
        .rules
        .iter()
        .find(|rule| rule.target == target && rule.kind == kind)
        .cloned();
    let id = uuid::Uuid::new_v4().to_string();
    let icon_dir = data_dir()?.join("icons");
    fs::create_dir_all(&icon_dir).map_err(|e| e.to_string())?;
    let png_path = icon_dir.join(format!("{id}.png"));
    let bytes = STANDARD
        .decode(png_data)
        .map_err(|e| format!("图片数据无效：{e}"))?;
    fs::write(&png_path, bytes).map_err(|e| format!("保存图标失败：{e}"))?;
    let native_path = prepare_native_icon(&png_path, &id)?;
    let targets = if kind == "folder" {
        collect_directories(Path::new(&target), &recursive_mode, max_depth)?
    } else {
        vec![PathBuf::from(&target)]
    };
    let mut applied_targets = Vec::with_capacity(targets.len());
    for path in targets {
        let value = path.to_string_lossy().into_owned();
        let previous_backup = existing.as_ref().and_then(|rule| {
            rule.applied_targets
                .iter()
                .find(|item| item.target == value)
                .and_then(|item| item.backup.clone())
        });
        let backup = apply_native(&value, &kind, &native_path, previous_backup.is_none())?;
        applied_targets.push(AppliedTarget {
            target: value,
            backup: previous_backup.or(backup),
        });
    }
    let backup = existing
        .as_ref()
        .and_then(|rule| rule.backup.clone())
        .or_else(|| applied_targets.first().and_then(|v| v.backup.clone()));
    let rule = IconRule {
        id,
        name,
        target,
        kind,
        icon_path: native_path.to_string_lossy().into_owned(),
        enabled: add_to_monitor,
        monitored: add_to_monitor,
        last_applied: Some(now()),
        status: Some("ok".into()),
        backup,
        recursive_mode,
        max_depth,
        applied_targets,
    };
    settings
        .rules
        .retain(|r| !(r.target == rule.target && r.kind == rule.kind));
    settings.rules.push(rule.clone());
    write_settings(&settings)?;
    Ok(rule)
}

fn validate_target(target: &str, kind: &str) -> Result<(), String> {
    match kind {
        "folder" if !Path::new(target).is_dir() => Err("所选目录不存在".into()),
        "file" if !Path::new(target).is_file() => Err("所选文件不存在".into()),
        "extension" if !target.starts_with('.') || target.len() < 2 => {
            Err("扩展名应以点开头，例如 .md".into())
        }
        "folder" | "file" | "extension" => Ok(()),
        _ => Err("不支持的目标类型".into()),
    }
}

fn prepare_native_icon(png: &Path, id: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let ico = png.with_file_name(format!("{id}.ico"));
        let image = image::open(png).map_err(|e| e.to_string())?;
        // ICO 单层尺寸上限为 256，使用高质量缩放并保留透明通道。
        image
            .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
            .save_with_format(&ico, image::ImageFormat::Ico)
            .map_err(|e| format!("生成 ICO 失败：{e}"))?;
        Ok(ico)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = id;
        Ok(png.to_path_buf())
    }
}

#[cfg(target_os = "macos")]
fn apply_native(
    target: &str,
    kind: &str,
    icon: &Path,
    _capture_backup: bool,
) -> Result<Option<String>, String> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::NSString;
    if kind == "extension" {
        return Err("macOS 不支持全局扩展名图标，请选择单个文件或目录".into());
    }
    let target = NSString::from_str(target);
    let icon_path = NSString::from_str(&icon.to_string_lossy());
    let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &icon_path)
        .ok_or("无法读取生成的图标")?;
    if NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        Some(&image),
        &target,
        NSWorkspaceIconCreationOptions(0),
    ) {
        Ok(None)
    } else {
        Err("Finder 未能应用图标".into())
    }
}

#[cfg(target_os = "windows")]
fn apply_native(
    target: &str,
    kind: &str,
    icon: &Path,
    capture_backup: bool,
) -> Result<Option<String>, String> {
    use std::os::windows::process::CommandExt;
    use winreg::{
        enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER},
        RegKey,
    };
    const NO_WINDOW: u32 = 0x08000000;
    if kind == "file" {
        return Err("Windows 单文件图标需使用资源管理器扩展；第一版请改用文件类型规则".into());
    }
    let backup;
    if kind == "folder" {
        let ini = Path::new(target).join("desktop.ini");
        backup = if capture_backup {
            fs::read(&ini).ok().map(|v| STANDARD.encode(v))
        } else {
            None
        };
        fs::write(
            &ini,
            format!("[.ShellClassInfo]\r\nIconResource={},0\r\n", icon.display()),
        )
        .map_err(|e| format!("写入 desktop.ini 失败：{e}"))?;
        Command::new("attrib")
            .args(["+h", "+s", ini.to_string_lossy().as_ref()])
            .creation_flags(NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        Command::new("attrib")
            .args(["+r", target])
            .creation_flags(NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
    } else {
        let ext = target.to_lowercase();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let classes = hkcu
            .create_subkey("Software\\Classes")
            .map_err(|e| e.to_string())?
            .0;
        let user_choice_prog_id = hkcu
            .open_subkey(format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{}\\UserChoice",
                ext
            ))
            .ok()
            .and_then(|key| key.get_value::<String, _>("ProgId").ok());
        let registered_prog_id = classes
            .open_subkey(&ext)
            .ok()
            .and_then(|key| key.get_value::<String, _>("").ok())
            .or_else(|| {
                RegKey::predef(HKEY_CLASSES_ROOT)
                    .open_subkey(&ext)
                    .ok()
                    .and_then(|key| key.get_value::<String, _>("").ok())
            });
        let prog_id = user_choice_prog_id.or(registered_prog_id);
        let old_direct_icon: Option<String> = classes
            .open_subkey(format!("{}\\DefaultIcon", ext))
            .ok()
            .and_then(|k| k.get_value("").ok());
        let old_prog_icon = prog_id.as_ref().and_then(|value| {
            classes
                .open_subkey(format!("{}\\DefaultIcon", value))
                .ok()
                .and_then(|key| key.get_value("").ok())
        });
        backup = if capture_backup {
            Some(
                serde_json::to_string(&WindowsExtensionBackup {
                    version: 1,
                    old_direct_icon,
                    prog_id: prog_id.clone(),
                    old_prog_icon,
                })
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        classes
            .create_subkey(format!("{}\\DefaultIcon", ext))
            .map_err(|e| e.to_string())?
            .0
            .set_value("", &format!("{},0", icon.display()))
            .map_err(|e| e.to_string())?;
        if let Some(value) = prog_id {
            classes
                .create_subkey(format!("{}\\DefaultIcon", value))
                .map_err(|e| e.to_string())?
                .0
                .set_value("", &format!("{},0", icon.display()))
                .map_err(|e| e.to_string())?;
        }
    }
    unsafe {
        windows_sys::Win32::UI::Shell::SHChangeNotify(
            windows_sys::Win32::UI::Shell::SHCNE_ASSOCCHANGED as i32,
            windows_sys::Win32::UI::Shell::SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
    let _ = Command::new("ie4uinit.exe")
        .arg("-show")
        .creation_flags(NO_WINDOW)
        .status();
    Ok(backup)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn apply_native(
    _target: &str,
    _kind: &str,
    _icon: &Path,
    _capture_backup: bool,
) -> Result<Option<String>, String> {
    Err("当前系统暂不支持应用图标".into())
}

#[tauri::command]
fn restore_icon(rule: IconRule) -> Result<(), String> {
    restore_native(&rule.target, &rule.kind, rule.backup.as_deref())
}

#[tauri::command]
fn delete_rule(id: String, restore: bool) -> Result<AppSettings, String> {
    let mut settings = read_settings();
    if let Some(rule) = settings.rules.iter().find(|r| r.id == id).cloned() {
        if restore {
            if rule.applied_targets.is_empty() {
                restore_native(&rule.target, &rule.kind, rule.backup.as_deref())?
            } else {
                for item in &rule.applied_targets {
                    restore_native(&item.target, &rule.kind, item.backup.as_deref())?
                }
            }
        }
        settings.rules.retain(|r| r.id != id);
        write_settings(&settings)?;
    }
    Ok(settings)
}

#[tauri::command]
fn toggle_rule(id: String, enabled: bool) -> Result<AppSettings, String> {
    let mut settings = read_settings();
    if let Some(rule) = settings.rules.iter_mut().find(|r| r.id == id) {
        rule.enabled = enabled;
    }
    write_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn restore_target(target: String, kind: String) -> Result<AppSettings, String> {
    validate_target(&target, &kind)?;
    let mut settings = read_settings();
    let matches: Vec<IconRule> = settings
        .rules
        .iter()
        .filter(|r| r.target == target && r.kind == kind)
        .cloned()
        .collect();
    if matches.is_empty() {
        restore_native(&target, &kind, None)?
    } else {
        for rule in &matches {
            if rule.applied_targets.is_empty() {
                restore_native(&rule.target, &rule.kind, rule.backup.as_deref())?
            } else {
                for item in &rule.applied_targets {
                    restore_native(&item.target, &rule.kind, item.backup.as_deref())?
                }
            }
        }
        settings
            .rules
            .retain(|r| !(r.target == target && r.kind == kind));
        write_settings(&settings)?;
    }
    Ok(settings)
}

#[cfg(target_os = "macos")]
fn restore_native(target: &str, kind: &str, _backup: Option<&str>) -> Result<(), String> {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::NSString;
    if kind == "extension" {
        return Ok(());
    }
    let target = NSString::from_str(target);
    if NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        None,
        &target,
        NSWorkspaceIconCreationOptions(0),
    ) {
        Ok(())
    } else {
        Err("Finder 未能恢复默认图标".into())
    }
}

#[cfg(target_os = "windows")]
fn restore_native(target: &str, kind: &str, backup: Option<&str>) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    if kind == "folder" {
        let ini = Path::new(target).join("desktop.ini");
        if let Some(data) = backup.and_then(|v| STANDARD.decode(v).ok()) {
            fs::write(ini, data).map_err(|e| e.to_string())?;
        } else if ini.exists() {
            fs::remove_file(ini).map_err(|e| e.to_string())?;
        }
    } else if kind == "extension" {
        let classes = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Classes", winreg::enums::KEY_ALL_ACCESS)
            .map_err(|e| e.to_string())?;
        let ext = target.to_lowercase();
        if let Some(current) =
            backup.and_then(|value| serde_json::from_str::<WindowsExtensionBackup>(value).ok())
        {
            let direct_icon = format!("{}\\DefaultIcon", ext);
            if let Some(value) = current.old_direct_icon {
                classes
                    .create_subkey(&direct_icon)
                    .map_err(|e| e.to_string())?
                    .0
                    .set_value("", &value)
                    .map_err(|e| e.to_string())?;
            } else {
                let _ = classes.delete_subkey_all(&direct_icon);
            }
            if let Some(prog_id) = current.prog_id {
                let prog_icon = format!("{}\\DefaultIcon", prog_id);
                if let Some(value) = current.old_prog_icon {
                    classes
                        .create_subkey(&prog_icon)
                        .map_err(|e| e.to_string())?
                        .0
                        .set_value("", &value)
                        .map_err(|e| e.to_string())?;
                } else {
                    let _ = classes.delete_subkey_all(&prog_icon);
                }
            }
        } else {
            let _ = classes.delete_subkey_all(format!("{}\\DefaultIcon", ext));
        }
        unsafe {
            windows_sys::Win32::UI::Shell::SHChangeNotify(
                windows_sys::Win32::UI::Shell::SHCNE_ASSOCCHANGED as i32,
                windows_sys::Win32::UI::Shell::SHCNF_IDLIST,
                std::ptr::null(),
                std::ptr::null(),
            );
        }
    }
    Ok(())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn restore_native(_target: &str, _kind: &str, _backup: Option<&str>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn repair_all() -> Result<usize, String> {
    let mut settings = read_settings();
    let mut count = 0;
    for rule in settings
        .rules
        .iter_mut()
        .filter(|r| r.monitored && r.enabled)
    {
        if validate_target(&rule.target, &rule.kind).is_err() {
            rule.enabled = false;
            rule.status = Some("目标已失效".into());
            continue;
        }
        if !Path::new(&rule.icon_path).exists() {
            rule.enabled = false;
            rule.status = Some("图标资源已丢失".into());
            continue;
        }
        {
            let targets = if rule.kind == "folder" {
                collect_directories(
                    Path::new(&rule.target),
                    &rule.recursive_mode,
                    rule.max_depth,
                )
                .unwrap_or_default()
            } else {
                vec![PathBuf::from(&rule.target)]
            };
            let mut failed = None;
            for target in targets {
                let value = target.to_string_lossy().into_owned();
                let known = rule.applied_targets.iter().any(|v| v.target == value);
                match apply_native(&value, &rule.kind, Path::new(&rule.icon_path), !known) {
                    Ok(backup) => {
                        if !known {
                            rule.applied_targets.push(AppliedTarget {
                                target: value,
                                backup,
                            });
                        }
                        count += 1
                    }
                    Err(e) => failed = Some(e),
                }
            }
            rule.status = Some(failed.unwrap_or_else(|| "ok".into()));
            rule.last_applied = Some(now());
        }
    }
    write_settings(&settings)?;
    Ok(count)
}

#[tauri::command]
fn check_rules() -> Result<AppSettings, String> {
    let mut settings = read_settings();
    for rule in &mut settings.rules {
        if validate_target(&rule.target, &rule.kind).is_err() {
            rule.enabled = false;
            rule.status = Some("目标已失效".into())
        } else if !Path::new(&rule.icon_path).exists() {
            rule.enabled = false;
            rule.status = Some("图标资源已丢失".into())
        }
    }
    write_settings(&settings)?;
    Ok(settings)
}

#[derive(Default)]
struct MonitorState {
    generation: Arc<Mutex<u64>>,
}
#[tauri::command]
fn configure_monitor(
    enabled: bool,
    interval_minutes: u64,
    state: tauri::State<MonitorState>,
) -> Result<AppSettings, String> {
    let mut settings = read_settings();
    settings.monitor_enabled = enabled;
    settings.monitor_interval_minutes = interval_minutes;
    write_settings(&settings)?;
    start_monitor(enabled, interval_minutes, &state);
    Ok(settings)
}
fn set_autostart_native(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let path = dirs::home_dir()
            .ok_or("无法确定用户目录")?
            .join("Library/LaunchAgents/com.iconcraft.desktop.plist");
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let safe = exe
                .to_string_lossy()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>Label</key><string>com.iconcraft.desktop</string><key>ProgramArguments</key><array><string>{safe}</string></array><key>RunAtLoad</key><true/></dict></plist>"#
            );
            fs::write(path, xml).map_err(|e| format!("设置开机自启失败：{e}"))?
        } else if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let run = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
            .map_err(|e| e.to_string())?
            .0;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            run.set_value("IconCraft", &format!("\"{}\"", exe.display()))
                .map_err(|e| e.to_string())?
        } else {
            let _ = run.delete_value("IconCraft");
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = enabled;
        Err("当前系统不支持开机自启".into())
    }
}
#[tauri::command]
fn configure_autostart(enabled: bool) -> Result<AppSettings, String> {
    set_autostart_native(enabled)?;
    let mut settings = read_settings();
    settings.autostart = enabled;
    write_settings(&settings)?;
    Ok(settings)
}
fn start_monitor(enabled: bool, interval_minutes: u64, state: &MonitorState) {
    let generation = {
        let mut value = state.generation.lock().unwrap();
        *value += 1;
        *value
    };
    if !enabled {
        return;
    }
    let shared = state.generation.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(interval_minutes.max(1) * 60));
        if *shared.lock().unwrap() != generation {
            break;
        }
        let _ = repair_all();
    });
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(MonitorState::default())
        .setup(|app| {
            let initial = read_settings();
            let open = MenuItem::with_id(app, "open", "打开 IconCraft", true, None::<&str>)?;
            let repair = MenuItem::with_id(app, "repair", "立即修复图标", true, None::<&str>)?;
            let protection = CheckMenuItem::with_id(
                app,
                "protection",
                "图标保护",
                true,
                initial.monitor_enabled,
                None::<&str>,
            )?;
            let autostart = CheckMenuItem::with_id(
                app,
                "autostart",
                "开机自启",
                true,
                initial.autostart,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &repair, &protection, &autostart, &quit])?;
            let tray_icon = app
                .default_window_icon()
                .cloned()
                .ok_or("应用图标加载失败")?;
            let protection_item = protection.clone();
            let autostart_item = autostart.clone();
            TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(false)
                .menu(&menu)
                .tooltip("IconCraft 图标工坊")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => show_window(app),
                    "repair" => {
                        let _ = repair_all();
                    }
                    "protection" => {
                        let enabled = protection_item.is_checked().unwrap_or(false);
                        let mut s = read_settings();
                        s.monitor_enabled = enabled;
                        let _ = write_settings(&s);
                        start_monitor(
                            enabled,
                            s.monitor_interval_minutes,
                            &app.state::<MonitorState>(),
                        );
                    }
                    "autostart" => {
                        let enabled = autostart_item.is_checked().unwrap_or(false);
                        let _ = set_autostart_native(enabled);
                        let mut s = read_settings();
                        s.autostart = enabled;
                        let _ = write_settings(&s);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;
            if initial.monitor_enabled {
                start_monitor(
                    true,
                    initial.monitor_interval_minutes,
                    &app.state::<MonitorState>(),
                );
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_platform,
            load_settings,
            save_settings,
            apply_icon,
            restore_icon,
            restore_target,
            preview_easter,
            apply_easter,
            restore_easter,
            delete_rule,
            toggle_rule,
            check_rules,
            repair_all,
            configure_monitor,
            configure_autostart
        ])
        .run(tauri::generate_context!())
        .expect("启动 IconCraft 失败");
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn macos_目录图标可以应用并恢复() {
        let target = std::env::temp_dir().join(format!("iconcraft-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&target).unwrap();
        let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.png");
        apply_native(target.to_str().unwrap(), "folder", &icon, true).unwrap();
        restore_native(target.to_str().unwrap(), "folder", None).unwrap();
        fs::remove_dir(&target).unwrap();
    }

    #[test]
    fn 递归层级限制准确() {
        let root = std::env::temp_dir().join(format!("iconcraft-tree-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("一级/二级/三级")).unwrap();
        assert_eq!(collect_directories(&root, "none", None).unwrap().len(), 1);
        assert_eq!(
            collect_directories(&root, "depth", Some(2)).unwrap().len(),
            3
        );
        assert_eq!(collect_directories(&root, "all", None).unwrap().len(), 4);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn 彩蛋范围统计准确() {
        let root = std::env::temp_dir().join(format!("iconcraft-easter-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("一级/二级")).unwrap();
        fs::write(root.join("根.txt"), b"test").unwrap();
        fs::write(root.join("一级/图片.PNG"), b"test").unwrap();
        fs::write(root.join("一级/二级/无扩展名"), b"test").unwrap();
        let (dirs, files, extensions) = collect_easter(&root, "all", None).unwrap();
        assert_eq!(dirs.len(), 3);
        assert_eq!(files.len(), 3);
        assert_eq!(extensions, vec![".png", ".txt"]);
        fs::remove_dir_all(root).unwrap();
    }
}
