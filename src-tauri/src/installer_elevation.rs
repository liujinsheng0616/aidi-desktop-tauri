/// 普通启动返回 740（需要提权）时请求 Windows 显示 UAC。
/// 调用方已尝试过普通启动且启动失败，因此这里不会重复运行已启动的安装包。
pub(crate) fn run_installer_as_admin(
    path: &std::path::Path,
    args: &[&str],
) -> Result<Option<u32>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_NOCLOSEPROCESS};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    struct ProcessHandle(HANDLE);
    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // 参数仅来自固定软件清单，不包含 AI 回复或用户输入。
    let parameters: Vec<u16> = args
        .join(" ")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW::default();
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = windows::core::w!("runas");
    info.lpFile = PCWSTR(file.as_ptr());
    info.lpParameters = if args.is_empty() {
        PCWSTR::null()
    } else {
        PCWSTR(parameters.as_ptr())
    };
    info.nShow = SW_SHOWNORMAL.0;

    if let Err(error) = unsafe { ShellExecuteExW(&mut info) } {
        if error.code().0 as u32 == 0x8007_04c7 {
            return Err("已取消管理员授权，安装程序未启动".to_string());
        }
        return Err(format!("请求管理员权限失败: {error}"));
    }
    if info.hProcess.is_invalid() {
        // Shell API 不保证每次都返回进程句柄；仍可依靠后续注册表检查。
        return Ok(None);
    }
    let handle = ProcessHandle(info.hProcess);
    if unsafe { WaitForSingleObject(handle.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err("等待管理员权限安装进程失败".to_string());
    }
    let mut exit_code = 0_u32;
    unsafe { GetExitCodeProcess(handle.0, &mut exit_code) }
        .map_err(|e| format!("读取安装进程结果失败: {e}"))?;
    Ok(Some(exit_code))
}
