fn main() {
    tonic_build::configure()
        .build_server(false)
        .compile(
            &["proto/sni.proto"],
            &["proto"],
        )
        .unwrap();
}