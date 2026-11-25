fn main() {
    // 将构建时的 AIDI_ENV 烧入二进制，运行时通过 env!("AIDI_ENV_BAKED") 读取
    let aidi_env = std::env::var("AIDI_ENV").unwrap_or_else(|_| "prod".to_string());
    println!("cargo:rustc-env=AIDI_ENV_BAKED={}", aidi_env);

    let mode = match aidi_env.as_str() {
        "dev" | "development" => "development",
        "test" => "test",
        _ => "production",
    };
    let env_file = format!("../.env.{}", mode);
    println!("cargo:rerun-if-changed={}", env_file);
    println!("cargo:rerun-if-env-changed=AIDI_ENV");
    println!("cargo:rerun-if-env-changed=VITE_TRACKER_BASE_URL");
    let tracker_url = std::env::var("VITE_TRACKER_BASE_URL").ok().or_else(|| {
        let contents = std::fs::read_to_string(env_file).ok()?;
        contents.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == "VITE_TRACKER_BASE_URL")
                .then(|| value.trim().trim_matches(&['"', '\''][..]).to_owned())
        })
    });
    if let Some(url) = tracker_url {
        println!("cargo:rustc-env=AIDI_TELEMETRY_URL={}", url);
    }

    tauri_build::build()
}
