fn main() {
    // Re-embed migrations when files are added without changing Rust sources.
    println!("cargo:rerun-if-changed=migrations");
}
