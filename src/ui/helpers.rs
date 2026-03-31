use crate::core::sftp::RemoteEntry;
use super::file_panel::{FileEntry, SortColumn};

pub fn format_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in &units {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

pub fn format_permissions(mode: u32) -> String {
    if mode == 0 {
        return String::new();
    }
    let mut s = String::with_capacity(9);
    let flags = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    for (bit, ch) in &flags {
        s.push(if mode & bit != 0 { *ch } else { '-' });
    }
    s
}

pub fn remote_to_file_entries(entries: &[RemoteEntry], show_hidden: bool) -> Vec<FileEntry> {
    let mut result = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
        permissions: String::new(),
    }];

    result.extend(entries.iter()
        .filter(|e| show_hidden || !e.name.starts_with('.'))
        .map(|e| FileEntry {
            name: e.name.clone(),
            is_dir: e.is_dir,
            size: e.size,
            modified: format_timestamp(e.mtime),
            permissions: format_permissions(e.permissions),
        }));

    result
}

pub fn parent_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => path[..i].to_string(),
        None => "/".to_string(),
    }
}

pub fn sort_entries(entries: &mut [FileEntry], column: SortColumn, ascending: bool) {
    if entries.len() <= 1 {
        return;
    }
    let rest = &mut entries[1..];
    rest.sort_by(|a, b| {
        let dir_cmp = b.is_dir.cmp(&a.is_dir);
        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }
        let cmp = match column {
            SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortColumn::Size => a.size.cmp(&b.size),
            SortColumn::Modified => a.modified.cmp(&b.modified),
            SortColumn::Permissions => a.permissions.cmp(&b.permissions),
        };
        if ascending { cmp } else { cmp.reverse() }
    });
}

pub fn read_local_dir(path: &str, show_hidden: bool) -> Vec<FileEntry> {
    let mut entries = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
        permissions: String::new(),
    }];

    if let Ok(read_dir) = std::fs::read_dir(path) {
        let mut items: Vec<FileEntry> = read_dir
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                let modified = meta
                    .modified()
                    .ok()
                    .map(|t| {
                        let elapsed = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        format_timestamp(elapsed)
                    })
                    .unwrap_or_default();

                #[cfg(unix)]
                let permissions = {
                    use std::os::unix::fs::PermissionsExt;
                    format_permissions(meta.permissions().mode())
                };
                #[cfg(not(unix))]
                let permissions = String::new();

                Some(FileEntry {
                    name: e.file_name().to_string_lossy().to_string(),
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                    modified,
                    permissions,
                })
            })
            .collect();

        items.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
        });

        if !show_hidden {
            items.retain(|e| !e.name.starts_with('.'));
        }

        entries.extend(items);
    }

    entries
}

pub fn format_timestamp(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let mut days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, m, d, hours, minutes)
}
