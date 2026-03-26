fn main() {
    // 将构建时的 AIDI_ENV 烧入二进制，运行时通过 env!("AIDI_ENV_BAKED") 读取
    let aidi_env = std::env::var("AIDI_ENV").unwrap_or_else(|_| "prod".to_string());
    println!("cargo:rustc-env=AIDI_ENV_BAKED={}", aidi_env);

    tauri_build::build()
}
