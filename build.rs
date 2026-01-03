fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
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
