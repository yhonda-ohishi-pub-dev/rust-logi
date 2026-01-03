fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    // Compile proto files with file descriptor for reflection
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .file_descriptor_set_path(out_dir.join("logi_descriptor.bin"))
        .compile_protos(
            &[
                "proto/common.proto",
                "proto/files.proto",
                "proto/car_inspection.proto",
                "proto/cam_files.proto",
                "proto/health.proto",
            ],
            &["proto"],
        )?;

    // Rerun if proto files change
    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}
