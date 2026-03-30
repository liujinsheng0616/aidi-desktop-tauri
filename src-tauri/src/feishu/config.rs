//! 飞书应用配置

/// 飞书应用 ID
pub const APP_ID: &str = "cli_a60fb1dcf8f4500d";

/// 飞书应用密钥
pub const APP_SECRET: &str = "GLki6s9Gr93lfvPkRvb4KeK6KLzMzQyH";

/// OAuth 回调地址
/// 开发时前端用 http://127.0.0.1:5173/oauth-callback.html，需在飞书控制台同时注册两个地址
/// 此处由 Rust 换 token 时发送，必须与前端授权时的 redirect_uri 完全一致
pub fn redirect_uri() -> &'static str {
    if cfg!(debug_assertions) {
        "http://127.0.0.1:5173/oauth-callback.html"
    } else {
        "https://aidi.yadea.com.cn/aidi-desktop/oauth-callback.html"
    }
}

/// 多维表格 app_token
pub const BITABLE_APP_TOKEN: &str = "ToysbAE0qa2P3ds9ze5cEbzdnbg";

/// 多维表格 table_id
pub const BITABLE_TABLE_ID: &str = "tblI9TMH7UPF0MQ1";

/// 飞书开放平台 API 基础 URL
pub const FEISHU_API_BASE: &str = "https://open.feishu.cn/open-apis";
