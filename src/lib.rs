use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, create_dir_all};
use std::io::Cursor;
use std::process::{Command, Stdio};
use tar::Archive;

#[derive(Default)]
pub struct Builder {
    pub name: String,
    pub version: String,
    pub tmp_dir: PathBuf,
    pub force_rebuild: bool,
    pub env: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: String,
    pub output_dir: Option<PathBuf>,
}
impl Builder {
    pub fn new(name: &str, version: &str, tmp_dir: &Path, target: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            tmp_dir: tmp_dir.to_path_buf(),
            target: target.to_string(),
            ..Default::default()
        }
    }

    pub fn build(self) -> Result<()> {
        let output_dir = self
            .output_dir
            .unwrap_or_else(|| PathBuf::from(std::env::var("OUT_DIR").unwrap()));

        BinCrate {
            name: self.name,
            version: self.version,
            tmp_dir: self.tmp_dir,
            force_rebuild: self.force_rebuild,
            envs: self.env,
            features: self.features,
            target: self.target,
            output_dir,
            ..Default::default()
        }
        .run()
    }
}

#[derive(Default)]
pub struct BinCrate {
    pub name: String,
    pub version: String,
    pub tmp_dir: PathBuf,
    pub force_rebuild: bool,
    pub envs: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: String,
    pub output_dir: PathBuf,
    crate_dir: PathBuf,
    base_dir: PathBuf,
}

impl BinCrate {
    pub fn run(&mut self) -> Result<()> {
        // 创建目标目录 target/tmp
        self.base_dir = self.tmp_dir.canonicalize().unwrap().join("tmp");
        create_dir_all(&self.base_dir).context("创建目录失败")?;
        // 构建 crate 唯一标识目录 (如 target/tmp/serde-1.0.0)
        self.crate_dir = self
            .base_dir
            .join(format!("{}-{}", self.name, self.version));

        // 检查是否已存在且不需要强制重建
        if self.crate_dir.exists() && !self.force_rebuild {
            println!("已存在缓存: {:?}", self.crate_dir);
        } else {
            // 清理旧目录 (如果存在)
            if self.crate_dir.exists() {
                fs::remove_dir_all(&self.crate_dir)?;
            }

            // 下载并解压源码
            self.download_crate()?;
        }

        self.build_crate()?;
        Ok(())
    }

    /// 下载并解压 crate 源码
    fn download_crate(&self) -> Result<()> {
        let url = format!(
            "https://crates.io/api/v1/crates/{}/{}/download",
            self.name, self.version
        );

        println!("正在下载: {url}");

        // 发送 HTTP 请求
        let response = reqwest::blocking::get(&url)?.error_for_status()?;
        let bytes = response.bytes()?;

        // 解压 gzip 流
        let tar = GzDecoder::new(Cursor::new(bytes));
        let mut archive = Archive::new(tar);

        // 解压到目标目录
        archive.unpack(&self.base_dir)?;
        println!("解压完成");

        Ok(())
    }

    /// 编译 crate 并返回可执行文件路径
    fn build_crate(&self) -> Result<()> {
        // 确保包含 Cargo.toml
        if !self.crate_dir.join("Cargo.toml").exists() {
            anyhow::bail!("目录中缺少 Cargo.toml: {:?}", self.crate_dir);
        }

        println!("开始编译...");

        let filtered_env: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH")
            .collect();

        let mut cargo = Command::new("cargo");

        cargo
            .args(["build", "-Z", "unstable-options", "--release", "--target"])
            .arg(&self.target)
            .arg("--artifact-dir")
            .arg(&self.output_dir)
            .env_clear()
            .envs(filtered_env)
            .current_dir(&self.crate_dir)
            .stdout(Stdio::inherit()) // 将输出传递到父进程
            .stderr(Stdio::inherit()); // 将错误传递到父进程

        for (key, value) in self.envs.iter() {
            cargo.env(key, value);
        }
        for f in self.features.iter() {
            cargo.arg("--features").arg(f);
        }

        println!("cmd: {:?}", cargo);

        // 执行 cargo build 并显示实时输出
        let status = cargo.status()?;

        if !status.success() {
            anyhow::bail!("编译失败: {}", status);
        }

        // // 查找生成的可执行文件
        // let exe_path = find_executable(crate_dir)?;

        // // 构建输出路径 (保留原始文件名)
        // let exe_name = exe_path
        //     .file_name()
        //     .ok_or_else(|| anyhow::anyhow!("无效的可执行文件路径"))?;

        // let output_path = output_dir.join(exe_name);

        // // 复制文件到指定位置
        // fs::copy(&exe_path, &output_path)?;
        // println!("已复制到: {}", output_path.display());

        // Ok(output_path)

        Ok(())
    }
}
