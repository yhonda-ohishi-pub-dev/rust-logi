fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    // Proto files are in packages/logi-proto/proto (shared with npm package)
    let proto_dir = "packages/logi-proto/proto";

    // Compile proto files with file descriptor for reflection
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .file_descriptor_set_path(out_dir.join("logi_descriptor.bin"))
        .compile_protos(
            &[
                format!("{}/common.proto", proto_dir),
                format!("{}/files.proto", proto_dir),
                format!("{}/car_inspection.proto", proto_dir),
                format!("{}/cam_files.proto", proto_dir),
                format!("{}/health.proto", proto_dir),
            ],
            &[proto_dir],
        )?;

    // Rerun if proto files change
    println!("cargo:rerun-if-changed={}/", proto_dir);

    Ok(())
}
