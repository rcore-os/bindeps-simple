use std::path::Path;

use bindeps_simple::Builder;

fn main() {
    let mf = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("manifest dir: {mf}");
    let mut builder = Builder::new("pie-boot-loader-aarch64");
    builder.manifest_path =
        Some("/home/zhourui/opensource/pie-boot/target/package/pie-boot-0.1.6/Cargo.toml".into());
    builder.user_crate_name = Some("pie-boot".into());
    builder.output_dir = Some(Path::new(&mf).join("target/tmp"));
    println!("building..");
    let out = builder
        .target("aarch64-unknown-none-softfloat")
        .cargo_args(&["-Z", "build-std=core,alloc"])
        .build()
        .unwrap();

    println!("building done {}", out.elf.display());
}
