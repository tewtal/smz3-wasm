fn main() {
    tonic_build::configure()
        .type_attribute(".", "#[derive(Serialize)]")
        .build_server(false)
        .compile(
            &["proto/randomizer.proto"],
            &["proto"],
        )
        .unwrap();
}