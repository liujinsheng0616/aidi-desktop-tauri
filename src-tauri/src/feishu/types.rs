//! 飞书 API 数据结构定义

use serde::{Deserialize, Serialize};

// ==================== 登录相关 ====================

/// OAuth token 请求响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub code: i32,
    pub msg: Option<String>,
    pub data: Option<OAuthTokenData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenData {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub open_id: String,
}

/// 用户信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoResponse {
    pub code: i32,
    pub msg: Option<String>,
    pub data: Option<UserInfoData>,
}

/// 飞书用户信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UserInfoData {
    pub name: Option<String>,
    pub en_name: Option<String>,
    pub avatar_url: Option<String>,
    pub open_id: Option<String>,
    pub union_id: Option<String>,
    pub email: Option<String>,
    pub mobile: Option<String>,
    pub user_id: Option<String>,
    pub employee_no: Option<String>,
    pub tenant_key: Option<String>,
}

// ==================== 租户 token ====================

/// 租户访问令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAccessTokenResponse {
    pub code: i32,
    pub msg: Option<String>,
    pub tenant_access_token: Option<String>,
    pub expire: Option<i64>,
}

// ==================== 多维表格 ====================

/// 多维表格记录创建响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitableRecordResponse {
    pub code: i32,
    pub msg: Option<String>,
    pub data: Option<BitableRecordData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitableRecordData {
    pub record: Option<BitableRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitableRecord {
    pub record_id: String,
    pub fields: serde_json::Value,
}

/// 搜索记录响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitableSearchResponse {
    pub code: i32,
    pub msg: Option<String>,
    pub data: Option<BitableSearchData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitableSearchData {
    pub items: Vec<BitableRecord>,
    pub total: i32,
}

// ==================== 设备信息上报 ====================

/// 设备信息上报请求
/// user_code: 员工工号（employee_no）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceReportRequest {
    /// 员工工号
    pub user_code: String,
    pub user_name: String,
    pub hostname: String,
    pub ip: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub manufacture_date: String,
    pub os_name: String,
    pub os_version: String,
    pub os_arch: String,
    pub os_install_date: String,
    pub os_last_boot: String,
    pub cpu_name: String,
    pub cpu_cores: i32,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub gpu_name: String,
    pub app_version: String,
}

impl DeviceReportRequest {
    /// 转换为飞书多维表格字段格式（字段名为中文，值全部转字符串）
    pub fn to_bitable_fields(&self) -> serde_json::Value {
        serde_json::json!({
            "员工工号": &self.user_code,
            "员工姓名": &self.user_name,
            "主机名": &self.hostname,
            "IP地址": &self.ip,
            "制造商": &self.manufacturer,
            "设备型号": &self.model,
            "序列号": &self.serial_number,
            "出厂日期": &self.manufacture_date,
            "操作系统": &self.os_name,
            "OS版本": &self.os_version,
            "系统架构": &self.os_arch,
            "安装时间": &self.os_install_date,
            "启动时间": &self.os_last_boot,
            "CPU型号": &self.cpu_name,
            "CPU核心数": self.cpu_cores.to_string(),
            "内存(GB)": self.memory_gb.to_string(),
            "存储(GB)": self.storage_gb.to_string(),
            "显卡型号": &self.gpu_name,
            "应用版本": &self.app_version,
            "上报时间": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }
}
