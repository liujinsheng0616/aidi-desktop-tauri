use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstallMode {
    #[allow(dead_code)]
    Interactive,
    Silent(&'static [&'static str]),
}

#[derive(Clone, Copy)]
struct InstallerDefinition {
    url: &'static str,
    mode: InstallMode,
}

impl InstallerDefinition {
    #[allow(dead_code)]
    const fn interactive(url: &'static str) -> Self {
        Self { url, mode: InstallMode::Interactive }
    }

    const fn silent(url: &'static str, args: &'static [&'static str]) -> Self {
        Self { url, mode: InstallMode::Silent(args) }
    }
}

// 与 Web 端 IT 场景中的公司软件清单保持一致。安装命令只接受这里列出的完整地址。
// 静默参数沿用旧版按文件名配置的参数；尚无 Windows 真机复测记录。
// PDF转换器旧版未配置静默参数，仍使用普通安装。
const INSTALLERS: &[InstallerDefinition] = &[
    InstallerDefinition::silent("http://10.149.5.210/windows/VNC-SERVER.exe", &["/quiet", "/norestart"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/WPS(推荐).exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/福昕PDF.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/搜狗拼音去广告.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/WinRAR.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/理光打印系统.exe", &["/quiet", "/norestart"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/WeChatWin.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/pdfgear_setup_v2.1.15.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/7z2201-x64.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/PotPlayerSetup64.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/腾讯会议.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/FreeCAD.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/剪映.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/影刀.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/VisualStudioSetup.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/CADReaderPc.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/teams.exe", &["/quiet", "/norestart"]),
    InstallerDefinition::interactive("http://10.149.5.210/windows/PDF转换器.exe"),
    InstallerDefinition::silent("http://10.149.5.210/windows/WorkBuddy.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/TraeCode_CN-Setup-x64.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/Zoom.exe", &["/quiet", "/norestart"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/豆包.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/搜狗 五笔版.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/Qoder.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/Postman (x64).exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/Git.exe", &["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/ChromeSetup.exe", &["/silent", "/install"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/钉钉.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/dbeaver.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/Foxmail.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/网易有道.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/OBS.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/kimi.exe", &["/S"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/飞书8.0.3.exe", &["/silent"]),
    InstallerDefinition::silent("http://10.149.5.210/windows/CitrixWorkspaceFullInstaller 2402 LTSR完整安装包.exe", &["/quiet", "/norestart"]),
];

pub(crate) fn installer_for_url(raw: &str) -> Result<(Url, InstallMode), String> {
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
    let installer = INSTALLERS.iter().find(|candidate| {
        Url::parse(candidate.url).map(|allowed| allowed == url).unwrap_or(false)
    }).ok_or("此安装地址不在公司软件清单中")?;
    Ok((url, installer.mode))
}

pub(crate) fn validate_installer_url(raw: &str) -> Result<Url, String> {
    installer_for_url(raw).map(|(url, _)| url)
}

pub(crate) fn silent_args(mode: InstallMode, force_interactive: bool) -> Option<&'static [&'static str]> {
    if force_interactive { return None; }
    match mode {
        InstallMode::Interactive => None,
        InstallMode::Silent(args) => Some(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_url_is_accepted() {
        assert_eq!(INSTALLERS.len(), 35);
        for installer in INSTALLERS {
            match installer.mode {
                InstallMode::Interactive => assert!(installer.url.ends_with("/PDF转换器.exe")),
                InstallMode::Silent(args) => assert!(!args.is_empty(), "{}", installer.url),
            }
            assert!(validate_installer_url(installer.url).is_ok(), "{}", installer.url);
            let encoded = Url::parse(installer.url).unwrap().to_string();
            assert!(validate_installer_url(&encoded).is_ok(), "{encoded}");
        }
    }

    #[test]
    fn silent_mode_requires_explicit_config_and_user_retry_disables_it() {
        let configured = InstallerDefinition::silent("http://example.invalid/test.exe", &["/S"]);
        assert_eq!(silent_args(configured.mode, false), Some(&["/S"][..]));
        assert_eq!(silent_args(configured.mode, true), None);
        assert_eq!(silent_args(InstallMode::Interactive, false), None);
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
