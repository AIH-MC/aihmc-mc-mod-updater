mod utils;

use clap::Parser;
use utils::index::ModrinthIndex;
use utils::{hasher, scanner, downloader};
use std::collections::{HashSet, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use anyhow::{Result, Context};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Local path or remote URL to modrinth.index.json
    #[arg(short, long, default_value = "modrinth.index.json")]
    index: String,

    /// Minecraft root directory
    #[arg(short, long, default_value = "")]
    game_dir: String,

    /// Folders to ignore (e.g., -e tacz -e resourcepacks)
    #[arg(short, long)]
    exclude: Vec<String>,

    /// aria2c: max-connection-per-server (-x)
    #[arg(short = 'x', long, default_value_t = 16)]
    connections: u32,

    /// aria2c: split (-s)
    #[arg(short = 's', long, default_value_t = 16)]
    split: u32,

    /// Preview changes and ask for confirmation
    #[arg(short, long)]
    preview: bool,
}

fn main() -> Result<()> {
    let mut args = Args::parse();

    if args.game_dir.is_empty() {
        args.game_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.to_string_lossy().to_string()))
            .unwrap_or_else(|| ".".to_string());
    }

    // 彻底解决路径问题：
    // 1. 先将所有反斜杠统一替换为正斜杠
    args.game_dir = args.game_dir.replace("\\", "/");
    
    // 2. 处理 Windows 命令行转义问题：当路径以 \" 结尾时，引号会被误认为是路径内容
    //    如果路径中包含双引号，则截取第一个双引号之前的内容
    if let Some(pos) = args.game_dir.find('"') {
        args.game_dir = args.game_dir[..pos].to_string();
    }

    // 3. 移除末尾多余的空格和正斜杠
    args.game_dir = args.game_dir.trim().trim_end_matches('/').to_string();

    for e in &mut args.exclude {
        let mut cleaned = e.replace("\\", "/");
        if let Some(pos) = cleaned.find('"') {
            cleaned = cleaned[..pos].to_string();
        }
        *e = cleaned.trim().trim_end_matches('/').to_string();
    }

    if !args.index.starts_with("http://") && !args.index.starts_with("https://") {
        let mut cleaned = args.index.replace("\\", "/");
        if let Some(pos) = cleaned.find('"') {
            cleaned = cleaned[..pos].to_string();
        }
        args.index = cleaned.trim().trim_end_matches('/').to_string();
    }

    let game_dir = Path::new(&args.game_dir);

    println!("--- Minecraft Mod 增量更新器 ---");
    println!("Minecraft 目录: {}", game_dir.display());
    if !args.exclude.is_empty() {
        println!("正在忽略目录: {:?}", args.exclude);
    }
    
    println!("正在从 {} 加载索引...", args.index);
    let index = load_index(&args.index)?;
    println!("索引加载成功: {} (版本: {})", index.name, index.version_id);

    println!("正在扫描本地文件...");
    let local_files = scanner::scan_local_files(game_dir, &args.exclude);
    
    let mut index_files_map = HashMap::new();
    for file in &index.files {
        let should_exclude = args.exclude.iter().any(|e| {
            let prefix = format!("{}/", e);
            file.path.starts_with(&prefix) || file.path == *e
        });

        if !should_exclude {
            index_files_map.insert(file.path.clone(), file.clone());
        }
    }

    let mut to_trash = Vec::new();
    let mut to_download_files = Vec::new(); // 改为存储 ModrinthFile
    let mut kept_count = 0;

    let mut local_paths_found = HashSet::new();

    // 检查本地已存在的文件
    for local_path in local_files {
        let rel_path = local_path.strip_prefix(game_dir).context("Failed to strip prefix")?;
        let path_str = rel_path.to_str().unwrap().replace("\\", "/");
        local_paths_found.insert(path_str.clone());

        if let Some(index_file) = index_files_map.get(&path_str) {
            let expected_sha512 = index_file.hashes.get("sha512").context("Index missing sha512")?;
            let actual_sha512 = hasher::calculate_sha512(&local_path)?;

            if actual_sha512.to_lowercase() != expected_sha512.to_lowercase() {
                to_trash.push(local_path.clone());
                to_download_files.push(index_file.clone());
            } else {
                kept_count += 1;
            }
        } else {
            to_trash.push(local_path);
        }
    }

    // 检查索引中存在但本地缺失的文件
    for (path, index_file) in &index_files_map {
        if !local_paths_found.contains(path) {
            to_download_files.push(index_file.clone());
        }
    }

    // 打印预览
    println!("\n--- 更新预览 ---");
    println!("[保持] {} 个文件", kept_count);
    
    if !to_trash.is_empty() {
        println!("\n[清理/更新] 以下 {} 个文件将移至回收站:", to_trash.len());
        for p in &to_trash {
            println!("  - {}", p.display());
        }
    }

    if !to_download_files.is_empty() {
        println!("\n[下载] 以下 {} 个文件将被下载:", to_download_files.len());
        for f in &to_download_files {
            println!("  - {} ({} 源)", f.path, f.downloads.len());
        }
    }

    if to_trash.is_empty() && to_download_files.is_empty() {
        println!("\n所有文件均已是最新，无需更新。");
        return Ok(());
    }

    // 预览模式确认
    if args.preview {
        print!("\n是否执行以上操作? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("操作已取消。");
            return Ok(());
        }
    }

    // 执行移动到回收站
    if !to_trash.is_empty() {
        println!("\n正在处理旧文件...");
        for path in to_trash {
            if path.exists() {
                if let Err(e) = trash::delete(&path) {
                    eprintln!("无法将 {} 移至回收站: {}，将尝试直接删除...", path.display(), e);
                    fs::remove_file(path)?;
                }
            }
        }
    }

    // 执行下载
    if !to_download_files.is_empty() {
        println!("\n正在启动下载任务...");
        let dl = downloader::Downloader::new(args.connections, args.split);
        let mut success_count = 0;
        let mut fail_count = 0;

        for file in &to_download_files {
            println!("\n正在处理: {}", file.path);
            let dest = game_dir.join(&file.path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }

            match dl.download_with_fallback(&file.downloads, &dest) {
                Ok(_) => {
                    println!("  [成功] 已完成: {}", file.path);
                    success_count += 1;
                }
                Err(e) => {
                    eprintln!("  [跳过] 下载失败 {}: {}", file.path, e);
                    fail_count += 1;
                }
            }
        }
        println!("\n下载汇总: {} 成功, {} 失败", success_count, fail_count);
    }

    println!("\n--- 更新完成 ---");
    Ok(())
}

fn load_index(path_or_url: &str) -> Result<ModrinthIndex> {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        let resp = reqwest::blocking::get(path_or_url)?.text()?;
        let index: ModrinthIndex = serde_json::from_str(&resp)?;
        Ok(index)
    } else {
        let content = fs::read_to_string(path_or_url)?;
        let index: ModrinthIndex = serde_json::from_str(&content)?;
        Ok(index)
    }
}
