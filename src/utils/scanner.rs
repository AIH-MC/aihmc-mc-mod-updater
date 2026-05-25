use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn scan_local_files(base_path: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let targets = vec![
        ("mods", vec![".jar", ".zip"]),
        ("resourcepacks", vec![".zip"]),
        ("tacz", vec![".zip"]),
    ];

    for (dir, exts) in targets {
        // 如果该目录在排除列表中，跳过
        if exclude.iter().any(|e| e == dir) {
            continue;
        }

        let path = base_path.join(dir);
        if path.exists() && path.is_dir() {
            for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let file_path = entry.path();
                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        let ext = format!(".{}", ext.to_lowercase());
                        if exts.contains(&ext.as_str()) {
                            files.push(file_path.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    files
}
