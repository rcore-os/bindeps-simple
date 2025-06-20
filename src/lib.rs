use std::path::PathBuf;

use anyhow::{Result, anyhow};
use std::process::{Command, Stdio};

#[derive(Default)]
pub struct Builder {
    pub name: String,
    pub force_rebuild: bool,
    pub env: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub cargo_args: Vec<String>,
    pub manifest_path: Option<PathBuf>,
    /// 调用Bindeps的crate名，用于测试
    pub user_crate_name: Option<String>,
}
impl Builder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
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

    pub fn cargo_args<T: AsRef<str>>(mut self, args: &[T]) -> Self {
        self.cargo_args
            .extend(args.iter().map(|s| s.as_ref().to_string()));
        self
    }

    pub fn cargo_arg<T: AsRef<str>>(mut self, arg: T) -> Self {
        self.cargo_args.push(arg.as_ref().to_string());
        self
    }

    pub fn target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    pub fn build(self) -> Result<Output> {
        let output_dir = self
            .output_dir
            .unwrap_or_else(|| PathBuf::from(std::env::var("OUT_DIR").unwrap()));

        let target = self.target.unwrap_or_else(|| {
            std::env::var("TARGET").expect("TARGET environment variable is not set")
        });

        BinCrate {
            name: self.name,
            force_rebuild: self.force_rebuild,
            envs: self.env,
            features: self.features,
            target,
            output_dir,
            cargo_args: self.cargo_args,
            manifest_path: self.manifest_path,
        }
        .run()
    }
}

#[derive(Default)]
pub struct BinCrate {
    pub name: String,
    pub force_rebuild: bool,
    pub envs: Vec<(String, String)>,
    pub features: Vec<String>,
    pub target: String,
    pub output_dir: PathBuf,
    cargo_args: Vec<String>,
    manifest_path: Option<PathBuf>,
}

impl BinCrate {
    pub fn run(&mut self) -> Result<Output> {
        let manifest_path = self
            .manifest_path
            .clone()
            .unwrap_or_else(|| std::env::var("CARGO_MANIFEST_PATH").unwrap().into());

        let mut cargo_metadata = cargo_metadata::MetadataCommand::new();

        let metadata = cargo_metadata.manifest_path(&manifest_path).exec()?;

        let package = metadata
            .packages
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&self.name))
            .ok_or_else(|| anyhow!("Cannot find package {}, is it has lib.rs?", &self.name))?;

        println!("mf: {}", package.manifest_path);
        let mf_path = package.manifest_path.as_str();
        if mf_path.contains(".cargo/registry") || mf_path.contains(".cargo\\registry") {
            println!("is online");
        } else {
            println!("is local");
            println!(
                "cargo:rerun-if-changed={}",
                package.manifest_path.parent().unwrap()
            );
        }

        self.manifest_path = Some(package.manifest_path.as_os_str().into());

        self.build_crate()?;
        let out_dir = self.output_dir.join("bin");
        Ok(Output {
            dir: out_dir.clone(),
            elf: out_dir.join(&self.name),
        })
    }

    /// 编译 crate 并返回可执行文件路径
    fn build_crate(&self) -> Result<()> {
        let manifest = self.manifest_path.as_ref().unwrap().clone();
        println!("开始编译...");

        let mut cargo = Command::new("cargo");

        cargo
            .arg("install")
            .arg(&self.name)
            .args(["--locked", "-Z", "unstable-options", "--target"])
            .arg(&self.target)
            .arg("--root")
            .arg(&self.output_dir)
            .arg("--path")
            .arg(manifest.parent().unwrap())
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTC_WORKSPACE_WRAPPER") // used by clippy
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

        println!("cmd: {cargo:?}");

        // 执行 cargo build 并显示实时输出
        let status = cargo.status()?;

        if !status.success() {
            anyhow::bail!("编译失败: {}", status);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Output {
    pub dir: PathBuf,
    pub elf: PathBuf,
}
