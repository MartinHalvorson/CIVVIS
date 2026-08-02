fn main() {
    // The promoted binary name (or its launch environment) supplies its
    // revision at runtime. Keeping that identity out of Cargo's inputs lets a
    // tool-only HEAD change reuse the optimized engine build.
}
