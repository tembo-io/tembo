fn main() {
    // Check if the console feature is enabled
    if std::env::var("CARGO_FEATURE_CONSOLE").is_ok() {
        println!("cargo:rustc-cfg=tokio_unstable");
    }
}
