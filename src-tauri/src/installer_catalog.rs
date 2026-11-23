use url::Url;

// 与 Web 端 IT 场景中的公司软件清单保持一致。安装命令只接受这里列出的完整地址。
const APPROVED_INSTALLER_URLS: &[&str] = &[
    "http://10.149.5.210/windows/VNC-SERVER.exe",
    "http://10.149.5.210/windows/WPS(推荐).exe",
    "http://10.149.5.210/windows/福昕PDF.exe",
    "http://10.149.5.210/windows/搜狗拼音去广告.exe",
    "http://10.149.5.210/windows/WinRAR.exe",
    "http://10.149.5.210/windows/理光打印系统.exe",
    "http://10.149.5.210/windows/WeChatWin.exe",
    "http://10.149.5.210/windows/pdfgear_setup_v2.1.15.exe",
    "http://10.149.5.210/windows/7z2201-x64.exe",
    "http://10.149.5.210/windows/PotPlayerSetup64.exe",
    "http://10.149.5.210/windows/腾讯会议.exe",
    "http://10.149.5.210/windows/FreeCAD.exe",
    "http://10.149.5.210/windows/剪映.exe",
    "http://10.149.5.210/windows/影刀.exe",
    "http://10.149.5.210/windows/VisualStudioSetup.exe",
    "http://10.149.5.210/windows/CADReaderPc.exe",
    "http://10.149.5.210/windows/teams.exe",
    "http://10.149.5.210/windows/PDF转换器.exe",
    "http://10.149.5.210/windows/WorkBuddy.exe",
    "http://10.149.5.210/windows/TraeCode_CN-Setup-x64.exe",
    "http://10.149.5.210/windows/Zoom.exe",
    "http://10.149.5.210/windows/豆包.exe",
    "http://10.149.5.210/windows/搜狗 五笔版.exe",
    "http://10.149.5.210/windows/Qoder.exe",
    "http://10.149.5.210/windows/Postman (x64).exe",
    "http://10.149.5.210/windows/Git.exe",
    "http://10.149.5.210/windows/ChromeSetup.exe",
    "http://10.149.5.210/windows/钉钉.exe",
    "http://10.149.5.210/windows/dbeaver.exe",
    "http://10.149.5.210/windows/Foxmail.exe",
    "http://10.149.5.210/windows/网易有道.exe",
    "http://10.149.5.210/windows/OBS.exe",
    "http://10.149.5.210/windows/kimi.exe",
    "http://10.149.5.210/windows/飞书8.0.3.exe",
    "http://10.149.5.210/windows/CitrixWorkspaceFullInstaller 2402 LTSR完整安装包.exe",
];

pub(crate) fn validate_installer_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "无效的软件安装地址".to_string())?;
    if url.scheme() != "http"
        || url.host_str() != Some("10.149.5.210")
        || url.port_or_known_default() != Some(80)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("此安装地址不在公司软件清单中".to_string());
    }
    let approved = APPROVED_INSTALLER_URLS.iter().any(|candidate| {
        Url::parse(candidate).map(|allowed| allowed == url).unwrap_or(false)
    });
    if !approved {
        return Err("此安装地址不在公司软件清单中".to_string());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_url_is_accepted() {
        assert_eq!(APPROVED_INSTALLER_URLS.len(), 35);
        for url in APPROVED_INSTALLER_URLS {
            assert!(validate_installer_url(url).is_ok(), "{url}");
            let encoded = Url::parse(url).unwrap().to_string();
            assert!(validate_installer_url(&encoded).is_ok(), "{encoded}");
        }
    }

    #[test]
    fn unapproved_sources_and_variants_are_rejected() {
        for url in [
            "https://evil.example/windows/WeChatWin.exe",
            "http://10.149.5.210.evil.example/windows/WeChatWin.exe",
            "http://10.149.5.210/windows/unknown.exe",
            "http://10.149.5.210/windows/Feilian_Windows_x86_v3.3.18_r5310_f39e3b.exe",
            "http://10.149.5.210/windows/WeChatWin.exe?file=other",
            "http://user@10.149.5.210/windows/WeChatWin.exe",
            "file:///C:/Windows/System32/calc.exe",
        ] {
            assert!(validate_installer_url(url).is_err(), "{url}");
        }
    }
}
