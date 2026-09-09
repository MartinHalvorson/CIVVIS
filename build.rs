fn main() {
    // The promoted binary name (or its launch environment) supplies its
    // revision at runtime. Keeping that identity out of Cargo's inputs lets a
    // tool-only HEAD change reuse the optimized engine build. Cargo otherwise
    // reruns even an empty build script for every package-file change. Rust
    // sources and include_str!/include_bytes! inputs remain compiler-tracked.
    println!("cargo:rerun-if-changed=build.rs");
}
