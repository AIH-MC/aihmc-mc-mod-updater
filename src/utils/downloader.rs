use std::process::Command;
use std::path::Path;
use anyhow::{Result, Context};

pub struct Downloader {
    aria2_path: String,
    connections: u32,
    split: u32,
}

impl Downloader {
    pub fn new(connections: u32, split: u32) -> Self {
        Self {
            aria2_path: "3rd/aria2c.exe".to_string(),
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
                    last_error = format!("无法执行 aria2c: {}", e);
                    println!("  [错误] 启动 aria2c 失败: {}", e);
                }
            }
        }

        Err(anyhow::anyhow!("所有下载源均失败。最后错误: {}", last_error))
    }
}
