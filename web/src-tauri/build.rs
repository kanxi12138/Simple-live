use std::fs;
use std::io::Result;

fn main() -> Result<()> {
    let out_path = "src/platforms/douyin/danmu/gen";
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("Failed to locate protoc");

    // Ensure the output directory exists
    fs::create_dir_all(out_path)?;
    std::env::set_var("PROTOC", protoc);

    prost_build::Config::new()
        .out_dir(out_path) // Specify the output directory within the project
        .compile_protos(
            &["src/platforms/douyin/danmu/douyin.proto"], // Corrected path
            &["src/platforms/douyin/danmu/"], // Kept include path, ensure it's correct for any imports in douyin.proto
        )
        .expect("Failed to compile danmu protos");

    tauri_build::build(); // Call this if it's needed by your Tauri setup, otherwise can be removed if you handle tauri specific build steps elsewhere.

    Ok(())
}
