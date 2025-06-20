use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use bindeps_simple::Builder;
use cargo_metadata::MetadataCommand;
use flate2::read::GzDecoder;
use std::io::Cursor;
use tar::Archive;

#[test]
fn test_local() {
    let manifest = get_local();

    let mf = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("manifest dir: {mf}");
    let mut builder = Builder::new("pie-boot-loader-aarch64");

    builder.manifest_path = Some(manifest);
    builder.user_crate_name = Some("pie-boot".into());
    builder.output_dir = Some(Path::new(&mf).join("target/tmp"));
    builder.target_dir = Some(Path::new(&mf).join("target"));

    println!("building..");
    builder
        .target("aarch64-unknown-none-softfloat")
        .cargo_args(&["-Z", "build-std=core,alloc"])
        .build()
        .unwrap();
}

#[test]
fn test_crate_io() {
    let manifest = get_cratesio();

    let mf = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("manifest dir: {mf}");
    let mut builder = Builder::new("pie-boot-loader-aarch64");

    builder.manifest_path = Some(manifest);
    builder.user_crate_name = Some("pie-boot".into());
    builder.output_dir = Some(Path::new(&mf).join("target/tmp"));
    builder.target_dir = Some(Path::new(&mf).join("target"));

    println!("building..");
    builder
        .target("aarch64-unknown-none-softfloat")
        .cargo_args(&["-Z", "build-std=core,alloc"])
        .build()
        .unwrap();
}

fn get_local() -> PathBuf {
    let my_meta = MetadataCommand::new().exec().unwrap();
    let target_dir = my_meta.target_directory.as_std_path();

    let src_dir = target_dir.join("pie-boot");

    if !src_dir.exists() {
        Command::new("git")
            .args(["clone", "https://github.com/rcore-os/pie-boot.git"])
            .stdout(Stdio::inherit())
            .current_dir(target_dir)
            .status()
            .unwrap();
    }

    src_dir.join("pie-boot").join("Cargo.toml")
}

fn get_cratesio() -> PathBuf {
    let version = "0.1.6";
    let url = format!("https://crates.io/api/v1/crates/pie-boot/{version}/download");

    // 发送 HTTP 请求
    let response = reqwest::blocking::get(url)
        .unwrap()
        .error_for_status()
        .unwrap();
    let bytes = response.bytes().unwrap();

    // 解压 gzip 流
    let tar = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(tar);

    let my_meta = MetadataCommand::new().exec().unwrap();
    let target_dir = my_meta.target_directory.as_std_path();

    // 解压到目标目录
    archive.unpack(target_dir).unwrap();
    println!("解压完成");

    target_dir
        .join(format!("pie-boot-{version}"))
        .join("Cargo.toml")
}
