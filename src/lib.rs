use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use flate2::read::GzDecoder;
use std::fs::{self, create_dir_all};
use std::io::Cursor;
use std::process::{Command, Stdio};
use tar::Archive;

#[derive(Default)]
pub struct Builder {
    pub name: String,
    pub version: String,
    pub force_rebuild: bool,
    pub env: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: String,
    pub output_dir: Option<PathBuf>,
    pub cargo_args: Vec<String>,
    pub source_dir: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
}
impl Builder {
    pub fn new(name: &str, version: &str, target: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            target: target.to_string(),
            ..Default::default()
        }
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    pub fn feature(mut self, feature: &str) -> Self {
        self.features.push(feature.to_string());
        self
    }

    pub fn source_dir<T: AsRef<Path>>(mut self, dir: T) -> Self {
        self.source_dir = Some(PathBuf::from(dir.as_ref()));
        self
    }

    pub fn cargo_args<T: AsRef<str>>(mut self, args: &[T]) -> Self {
        self.cargo_args
            .extend(args.iter().map(|s| s.as_ref().to_string()));
        self
    }

    pub fn cargo_arg<T: AsRef<str>>(mut self, arg: T) -> Self {
        self.cargo_args.push(arg.as_ref().to_string());
        self
    }

    pub fn build(self) -> Result<()> {
        let output_dir = self
            .output_dir
            .unwrap_or_else(|| PathBuf::from(std::env::var("OUT_DIR").unwrap()));

        BinCrate {
            name: self.name,
            version: self.version,
            force_rebuild: self.force_rebuild,
            envs: self.env,
            features: self.features,
            target: self.target,
            output_dir,
            cargo_args: self.cargo_args,
            source_dir: self.source_dir,
            manifest_path: self.manifest_path,
            ..Default::default()
        }
        .run()
    }
}

#[derive(Default)]
pub struct BinCrate {
    pub name: String,
    pub version: String,
    pub force_rebuild: bool,
    pub envs: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: String,
    pub output_dir: PathBuf,
    source_dir: Option<PathBuf>,
    cargo_args: Vec<String>,
    crate_dir: PathBuf,
    base_dir: PathBuf,
    manifest_path: Option<PathBuf>,
}

impl BinCrate {
    pub fn run(&mut self) -> Result<()> {
        if let Some(mf) = &self.manifest_path {
            let metadata = MetadataCommand::new()
                .manifest_path(mf)
                .no_deps()
                .exec()
                .unwrap();

            for pkg in metadata.packages {
                println!("   dep: {}", pkg.name);
                if p.name.eq_ignore_ascii_case(&self.name) {
                    println!("   {} manifest at: {:?}", self.name, pkg.manifest_path);
                }
            }
        }

        self.base_dir = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join("rust-bindeps-simple");
        println!("tmp  dir: {}", self.base_dir.display());
        create_dir_all(&self.base_dir).context("创建目录失败")?;

        if let Some(source_dir) = &self.source_dir {
            use rand::seq::IndexedRandom;
            let mut rng = &mut rand::rng();
            let sample =
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes();
            let suffix = sample
                .choose_multiple(&mut rng, 12)
                .cloned()
                .collect::<Vec<_>>();

            let suffix = String::from_utf8_lossy(&suffix);

            // 构建 crate 唯一标识目录 (如 target/tmp/serde-1.0.0)
            self.crate_dir = self.base_dir.join(format!("{}-{}", self.name, suffix));
            if self.crate_dir.exists() {
                // 删除目录
                std::fs::remove_dir_all(&self.crate_dir).unwrap();
            }
            std::fs::create_dir_all(&self.crate_dir).unwrap();

            // 复制 source_dir 内容到 crate_dir
            copy_dir_recursive(source_dir, &self.crate_dir)?;
        } else {
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
            .arg("-p")
            .arg(&self.name)
            .arg("--target-dir")
            .arg(self.output_dir.join(format!("{}-target", self.name)))
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

        for a in self.cargo_args.iter() {
            cargo.arg(a);
        }

        println!("cmd: {:?}", cargo);

        // 执行 cargo build 并显示实时输出
        let status = cargo.status()?;

        if !status.success() {
            anyhow::bail!("编译失败: {}", status);
        }

        Ok(())
    }
}

// 递归复制函数
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
