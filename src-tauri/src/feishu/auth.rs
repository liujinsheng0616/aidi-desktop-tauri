//! 飞书 OAuth 登录 API

use crate::feishu::config::{APP_ID, APP_SECRET, redirect_uri, FEISHU_API_BASE};
use crate::feishu::types::{OAuthTokenResponse, UserInfoData, UserInfoResponse};

/// 通过授权码获取用户访问令牌
async fn get_user_access_token(code: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let url = format!("{}/authen/v2/oauth/token", FEISHU_API_BASE);

    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": APP_ID,
        "client_secret": APP_SECRET,
        "code": code,
        "redirect_uri": redirect_uri(),
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求飞书 API 失败: {}", e))?;

    let token_resp: OAuthTokenResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if token_resp.code != 0 {
        return Err(format!(
            "获取 token 失败: {}",
            token_resp.msg.unwrap_or_else(|| token_resp.code.to_string())
        ));
    }

    token_resp
        .access_token
        .ok_or_else(|| "响应中缺少 access_token".to_string())
}

/// 通过用户访问令牌获取用户信息
async fn get_user_info(access_token: &str) -> Result<UserInfoData, String> {
    let client = reqwest::Client::new();

    let url = format!("{}/authen/v1/user_info", FEISHU_API_BASE);

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("请求用户信息失败: {}", e))?;

    let user_resp: UserInfoResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if user_resp.code != 0 {
        return Err(format!(
            "获取用户信息失败: {}",
            user_resp.msg.unwrap_or_else(|| user_resp.code.to_string())
        ));
    }

    user_resp.data.ok_or_else(|| "响应中缺少用户数据".to_string())
}

/// 一站式登录：code -> 用户信息
///
/// 内部完成：
/// 1. code -> user_access_token
/// 2. user_access_token -> 用户信息
pub async fn login(code: &str) -> Result<UserInfoData, String> {
    // Step 1: 获取用户访问令牌
    let access_token = get_user_access_token(code).await?;

    // Step 2: 获取用户信息
    let user_info = get_user_info(&access_token).await?;

    Ok(user_info)
}

/// Tauri 命令：飞书登录
#[tauri::command]
pub async fn feishu_login(code: String) -> Result<UserInfoData, String> {
    login(&code).await
}
