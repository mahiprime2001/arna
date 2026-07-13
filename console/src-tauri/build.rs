fn main() {
    // The agent bakes ARNA_DEFAULT_BACKEND at compile time via option_env!; tell
    // cargo to rebuild when it changes so CI can produce builds with different
    // baked-in servers without serving a stale cache.
    println!("cargo:rerun-if-env-changed=ARNA_DEFAULT_BACKEND");
    tauri_build::build()
}
