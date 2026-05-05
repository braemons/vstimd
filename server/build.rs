fn main() {
    prost_build::Config::new()
        .compile_protos(&["proto/wonderlamp.proto"], &["proto/"])
        .expect("failed to compile protobuf schema");

    compile_shader_to_spirv();
}

fn compile_shader_to_spirv() {
    let wgsl_path = "shaders/solid.wgsl";
    println!("cargo:rerun-if-changed={wgsl_path}");

    let wgsl = std::fs::read_to_string(wgsl_path)
        .expect("failed to read shaders/solid.wgsl");

    let module = naga::front::wgsl::parse_str(&wgsl)
        .expect("failed to parse WGSL shader");

    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("WGSL shader validation failed");

    let options = naga::back::spv::Options {
        lang_version: (1, 0),
        ..Default::default()
    };

    let spv_words = naga::back::spv::write_vec(&module, &info, &options, None)
        .expect("failed to write SPIR-V");

    let spv_bytes: Vec<u8> = spv_words.iter().flat_map(|&w| w.to_le_bytes()).collect();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out_dir}/solid.spv"), &spv_bytes)
        .expect("failed to write solid.spv");
}
