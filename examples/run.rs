use std::path::Path;

use bindeps_simple::Builder;

fn main() {
    let mf = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("manifest dir: {mf}");
    let mut builder = Builder::new(
        "pie-boot-loader-aarch64",
        "0.1.2",
        "aarch64-unknown-none-softfloat",
    );
    builder.output_dir = Some(Path::new(&mf).join("target/tmp"));
    println!("building..");
    builder.build().unwrap();
}
