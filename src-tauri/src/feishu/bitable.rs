//! 飞书多维表格 API

use std::time::{Duration, Instant};

use crate::feishu::config::{APP_ID, APP_SECRET, BITABLE_APP_TOKEN, BITABLE_TABLE_ID, FEISHU_API_BASE};
use crate::feishu::types::{BitableRecordResponse, BitableSearchResponse, DeviceReportRequest, TenantAccessTokenResponse};

/// 缓存的租户访问令牌
static TENANT_TOKEN_CACHE: std::sync::OnceLock<TenantTokenCache> = std::sync::OnceLock::new();

struct TenantTokenCache {
    token: std::sync::Mutex<Option<(String, Instant)>>,
}

impl TenantTokenCache {
    fn new() -> Self {
        Self {
            token: std::sync::Mutex::new(None),
        }
    }

    fn get(&self) -> Option<String> {
        let guard = self.token.lock().unwrap();
        if let Some((token, expires_at)) = guard.as_ref() {
            // 提前 5 分钟过期，避免临界情况
            if Instant::now() < *expires_at {
                return Some(token.clone());
            }
        }
        None
    }

    fn set(&self, token: String, expires_in: i64) {
        let mut guard = self.token.lock().unwrap();
        // 提前 5 分钟过期
        let expires_at = Instant::now() + Duration::from_secs(expires_in as u64 - 300);
        *guard = Some((token, expires_at));
    }
}

/// 获取租户访问令牌（带缓存）
async fn get_tenant_access_token() -> Result<String, String> {
    // 检查缓存
    let cache = TENANT_TOKEN_CACHE.get_or_init(TenantTokenCache::new);
    if let Some(token) = cache.get() {
        return Ok(token);
    }

    // 缓存未命中，请求新令牌
    let client = reqwest::Client::new();

    let url = format!("{}/auth/v3/tenant_access_token/internal", FEISHU_API_BASE);

    let body = serde_json::json!({
        "app_id": APP_ID,
        "app_secret": APP_SECRET,
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求租户令牌失败: {}", e))?;

    let token_resp: TenantAccessTokenResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if token_resp.code != 0 {
        return Err(format!(
            "获取租户令牌失败: {}",
            token_resp.msg.unwrap_or_else(|| token_resp.code.to_string())
        ));
    }

    let token = token_resp.tenant_access_token.ok_or("响应中缺少令牌")?;
    let expire = token_resp.expire.unwrap_or(7200);

    // 存入缓存
    cache.set(token.clone(), expire);

    Ok(token)
}

/// 查询记录（根据员工工号 + 序列号）
async fn find_record(user_code: &str, serial_number: &str) -> Result<Option<String>, String> {
    let app_token = BITABLE_APP_TOKEN;
    let table_id = BITABLE_TABLE_ID;

    let tenant_token = get_tenant_access_token().await?;
    let client = reqwest::Client::new();

    // 使用 List Records API 配合 view_id 获取所有记录，然后在代码中过滤
    // 或者尝试使用正确的 filter 格式
    let url = format!(
        "{}/bitable/v1/apps/{}/tables/{}/records",
        FEISHU_API_BASE, app_token, table_id
    );

    // 尝试不带 filter 先获取所有记录
    let response = client
        .get(&url)
        .bearer_auth(&tenant_token)
        .query(&[("page_size", "500")])
        .send()
        .await
        .map_err(|e| format!("查询记录失败: {}", e))?;

    let list_resp: BitableSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if list_resp.code != 0 {
        return Err(format!("查询记录失败: {}", list_resp.msg.unwrap_or_default()));
    }

    // 在代码中过滤匹配的记录
    if let Some(data) = &list_resp.data {
        for record in &data.items {
            if let Some(fields) = record.fields.as_object() {
                let record_user_code = fields.get("员工工号")
                    .and_then(|v| {
                        // 可能是字符串或字符串数组
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else if let Some(arr) = v.as_array() {
                            arr.first().and_then(|s| s.as_str()).map(|s| s.to_string())
                        } else {
                            None
                        }
                    });
                let record_serial = fields.get("序列号")
                    .and_then(|v| {
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else if let Some(arr) = v.as_array() {
                            arr.first().and_then(|s| s.as_str()).map(|s| s.to_string())
                        } else {
                            None
                        }
                    });

                if record_user_code.as_deref() == Some(user_code)
                    && record_serial.as_deref() == Some(serial_number) {
                    println!("[Bitable] 找到匹配记录: {}", record.record_id);
                    return Ok(Some(record.record_id.clone()));
                }
            }
        }
    }

    println!("[Bitable] 未找到匹配记录");
    Ok(None)
}

/// 更新记录
async fn update_record(record_id: &str, fields: serde_json::Value) -> Result<String, String> {
    let app_token = BITABLE_APP_TOKEN;
    let table_id = BITABLE_TABLE_ID;

    let tenant_token = get_tenant_access_token().await?;
    let client = reqwest::Client::new();

    let url = format!(
        "{}/bitable/v1/apps/{}/tables/{}/records/{}",
        FEISHU_API_BASE, app_token, table_id, record_id
    );

    let body = serde_json::json!({
        "fields": fields,
    });

    let response = client
        .put(&url)
        .bearer_auth(&tenant_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("更新记录失败: {}", e))?;

    let record_resp: BitableRecordResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if record_resp.code != 0 {
        return Err(format!("更新记录失败: {}", record_resp.msg.unwrap_or_default()));
    }

    Ok(record_id.to_string())
}

/// 向多维表格添加记录
async fn create_record(fields: serde_json::Value) -> Result<String, String> {
    let app_token = BITABLE_APP_TOKEN;
    let table_id = BITABLE_TABLE_ID;

    if app_token.is_empty() || table_id.is_empty() {
        return Err("多维表格配置缺失：请设置 BITABLE_APP_TOKEN 和 BITABLE_TABLE_ID".to_string());
    }

    // 获取租户令牌
    let tenant_token = get_tenant_access_token().await?;

    let client = reqwest::Client::new();

    let url = format!(
        "{}/bitable/v1/apps/{}/tables/{}/records",
        FEISHU_API_BASE, app_token, table_id
    );

    let body = serde_json::json!({
        "fields": fields,
    });

    let response = client
        .post(&url)
        .bearer_auth(&tenant_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("创建记录失败: {}", e))?;

    let record_resp: BitableRecordResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    if record_resp.code != 0 {
        return Err(format!(
            "创建记录失败: {}",
            record_resp.msg.unwrap_or_else(|| record_resp.code.to_string())
        ));
    }

    record_resp
        .data
        .and_then(|d| d.record.map(|r| r.record_id))
        .ok_or_else(|| "响应中缺少记录 ID".to_string())
}

/// 上报设备信息到飞书多维表格（先查询，存在则更新，不存在则创建）
pub async fn report_device(info: &DeviceReportRequest) -> Result<String, String> {
    let fields = info.to_bitable_fields();

    // 先查询是否存在（员工工号 + 序列号）
    match find_record(&info.user_code, &info.serial_number).await {
        Ok(Some(record_id)) => {
            // 存在，更新记录
            println!("[Bitable] 记录已存在，更新: {}", record_id);
            update_record(&record_id, fields).await
        }
        Ok(None) => {
            // 不存在，创建新记录
            println!("[Bitable] 记录不存在，创建新记录");
            create_record(fields).await
        }
        Err(e) => {
            // 查询失败，尝试直接创建
            println!("[Bitable] 查询失败: {}，尝试直接创建", e);
            create_record(fields).await
        }
    }
}

/// Tauri 命令：上报设备信息
#[tauri::command]
pub async fn feishu_report_device(info: DeviceReportRequest) -> Result<String, String> {
    report_device(&info).await
}
