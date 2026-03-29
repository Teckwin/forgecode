fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("proto/forge.proto")?;

    // Embed sample config as string when feature is enabled
    #[cfg(feature = "include_sample_config")]
    {
        let config = include_str!("resources/sample_config.yaml");
        println!("cargo:rustc-cfg=has_sample_config");
        println!("cargo:rerun-if-changed=resources/sample_config.yaml");
    }

    Ok(())
}
