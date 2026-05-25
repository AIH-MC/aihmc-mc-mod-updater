use std::process::Command;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use std::env;

pub struct Downloader {
    aria2_path: PathBuf,
    connections: u32,
    split: u32,
}

impl Downloader {
    pub fn new(connections: u32, split: u32) -> Self {
        let exe_path = env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
        let aria2_path = exe_dir.join("3rd").join("aria2c.exe");
        
        // 如果在开发环境下（target/debug），可能需要向上找一级或直接使用当前工作目录
        let aria2_path = if !aria2_path.exists() {
            let work_dir_path = Path::new("3rd/aria2c.exe");
            if work_dir_path.exists() {
                work_dir_path.to_path_buf()
            } else {
                aria2_path
            }
        } else {
            aria2_path
        };

        Self {
            aria2_path,
            connections,
            split,
        }
    }

    /// 尝试下载单个文件，如果失败则尝试下一个源
    pub fn download_with_fallback(&self, urls: &[String], dest_path: &Path) -> Result<()> {
        let dir = dest_path.parent().context("Invalid path")?.to_str().context("Non-utf8 path")?;
        let out = dest_path.file_name().context("Invalid filename")?.to_str().context("Non-utf8 filename")?;

        let mut last_error = "No URLs provided".to_string();

        for (i, url) in urls.iter().enumerate() {
            println!("  [源 {}/{}] 正在尝试: {}", i + 1, urls.len(), url);
            
            let status = Command::new(&self.aria2_path)
                .arg(url)
                .arg("--dir")
                .arg(dir)
                .arg("--out")
                .arg(out)
                .arg("--allow-overwrite=true")
                .arg("--check-certificate=false")
                .arg(format!("--max-connection-per-server={}", self.connections))
                .arg(format!("--split={}", self.split))
                .arg("--summary-interval=0")
                .arg("--console-log-level=warn")
                .status();

            match status {
                Ok(s) if s.success() => return Ok(()),
                Ok(s) => {
                    last_error = format!("aria2c 退出代码: {}", s);
                    println!("  [失败] 该源不可用，尝试下一个...");
                }
                Err(e) => {
                    last_error = format!("无法执行 aria2c: {} (路径: {})", e, self.aria2_path.display());
                    println!("  [错误] 启动 aria2c 失败: {}", e);
                }
            }
        }

        Err(anyhow::anyhow!("所有下载源均失败。最后错误: {}", last_error))
    }
}
