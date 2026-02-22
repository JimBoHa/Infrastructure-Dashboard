use anyhow::{bail, Context, Result};

use crate::utils::CommandResult;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libc::FILE;
    use std::ffi::{c_char, c_void, CString};
    use std::path::Path;
    use std::ptr;

    type AuthorizationRef = *mut c_void;
    type OSStatus = i32;
    type AuthorizationFlags = u32;

    const K_AUTHORIZATION_FLAG_INTERACTION_ALLOWED: AuthorizationFlags = 1 << 0;
    const K_AUTHORIZATION_FLAG_EXTEND_RIGHTS: AuthorizationFlags = 1 << 1;
    const K_AUTHORIZATION_FLAG_PRE_AUTHORIZE: AuthorizationFlags = 1 << 2;
    const K_AUTHORIZATION_FLAG_DESTROY_RIGHTS: AuthorizationFlags = 1 << 3;

    #[link(name = "Security", kind = "framework")]
    extern "C" {
        fn AuthorizationCreate(
            rights: *const c_void,
            environment: *const c_void,
            flags: AuthorizationFlags,
            authorization: *mut AuthorizationRef,
        ) -> OSStatus;

        fn AuthorizationFree(
            authorization: AuthorizationRef,
            flags: AuthorizationFlags,
        ) -> OSStatus;

        fn AuthorizationExecuteWithPrivileges(
            authorization: AuthorizationRef,
            path_to_tool: *const c_char,
            options: AuthorizationFlags,
            arguments: *mut *mut c_char,
            communications_pipe: *mut *mut FILE,
        ) -> OSStatus;
    }

    fn shell_escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('\'');
        for ch in value.chars() {
            if ch == '\'' {
                out.push_str("'\\''");
            } else {
                out.push(ch);
            }
        }
        out.push('\'');
        out
    }

    fn read_pipe(pipe: *mut FILE) -> Vec<u8> {
        if pipe.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = unsafe { libc::fread(buf.as_mut_ptr() as *mut c_void, 1, buf.len(), pipe) };
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n as usize]);
        }
        unsafe {
            libc::fclose(pipe);
        }
        out
    }

    fn parse_exit_code(output: &str) -> (i32, String) {
        let marker = "__FARMCTL_EXIT__:";
        if let Some(index) = output.rfind(marker) {
            let after = &output[index + marker.len()..];
            let code_str = after
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(|c: char| c == '\r' || c == '\n');
            let code = code_str.parse::<i32>().unwrap_or(-1);
            let cleaned = output[..index].trim_end().to_string();
            return (code, cleaned);
        }
        (-1, output.trim_end().to_string())
    }

    fn authorization_hint(status: OSStatus, action: &str) -> Option<&'static str> {
        match status {
            // Observed in production when the setup-daemon is running as a system LaunchDaemon and
            // cannot show GUI auth prompts.
            -60007 => Some(
                "Authorization prompts are not available in this context (likely a headless LaunchDaemon).\n\
Run the installer app (interactive admin prompt) for installs/uninstalls, or perform upgrades/rollbacks as the service user (via the setup-daemon).",
            ),
            // Observed when incorrect flag usage prevented the prompt from appearing.
            -60011 => Some(
                "Authorization flags were rejected. If this persists, ensure you are running from an interactive user session (not a LaunchDaemon).",
            ),
            _ => match action {
                "install" | "upgrade" | "rollback" | "uninstall" => Some(
                    "This action requires admin privileges. Run from an interactive user session (installer app / Terminal) so macOS can show the auth prompt, or run with sudo.",
                ),
                _ => None,
            },
        }
    }

    pub fn run_farmctl_authorized(
        farmctl_path: &str,
        args: &[&str],
        config_path: &Path,
        env_overrides: &[(&str, &str)],
    ) -> Result<CommandResult> {
        let action = args.first().copied().unwrap_or("");
        let mut cmd_display = format!("[authorized] {}", farmctl_path);
        for arg in args {
            cmd_display.push(' ');
            cmd_display.push_str(arg);
        }
        cmd_display.push_str(" --config ");
        cmd_display.push_str(&config_path.display().to_string());

        let mut env_prefix = String::new();
        for (key, value) in env_overrides {
            env_prefix.push_str(key);
            env_prefix.push('=');
            env_prefix.push_str(&shell_escape(value));
            env_prefix.push(' ');
        }

        let mut farmctl_cmd = String::new();
        farmctl_cmd.push_str(&env_prefix);
        farmctl_cmd.push_str(&shell_escape(farmctl_path));
        for arg in args {
            farmctl_cmd.push(' ');
            farmctl_cmd.push_str(&shell_escape(arg));
        }
        farmctl_cmd.push_str(" --config ");
        farmctl_cmd.push_str(&shell_escape(&config_path.display().to_string()));

        // Ensure we always emit an exit marker even if the command fails.
        let wrapped = format!("({farmctl_cmd}) 2>&1; echo \"__FARMCTL_EXIT__:$?\"");

        let tool = CString::new("/bin/sh").unwrap();
        let arg_flag = CString::new("-lc").unwrap();
        let arg_cmd = CString::new(wrapped).context("failed to build privileged shell command")?;

        let mut argv_storage = vec![arg_flag, arg_cmd];
        let mut argv: Vec<*mut c_char> = argv_storage
            .iter_mut()
            .map(|arg| arg.as_ptr() as *mut c_char)
            .collect();
        argv.push(ptr::null_mut());

        let mut auth: AuthorizationRef = ptr::null_mut();
        let flags = K_AUTHORIZATION_FLAG_INTERACTION_ALLOWED
            | K_AUTHORIZATION_FLAG_EXTEND_RIGHTS
            | K_AUTHORIZATION_FLAG_PRE_AUTHORIZE;
        let status = unsafe { AuthorizationCreate(ptr::null(), ptr::null(), flags, &mut auth) };
        if status != 0 {
            if let Some(hint) = authorization_hint(status, action) {
                bail!("AuthorizationCreate failed (status={status}).\n{hint}");
            }
            bail!("AuthorizationCreate failed (status={status})");
        }

        let mut pipe: *mut FILE = ptr::null_mut();
        // `AuthorizationExecuteWithPrivileges` expects `options` to be 0; passing the same flag set
        // as `AuthorizationCreate` yields `errAuthorizationInvalidFlags` (-60011) and no prompt.
        let exec_status = unsafe {
            AuthorizationExecuteWithPrivileges(auth, tool.as_ptr(), 0, argv.as_mut_ptr(), &mut pipe)
        };
        let _ = unsafe { AuthorizationFree(auth, K_AUTHORIZATION_FLAG_DESTROY_RIGHTS) };
        if exec_status != 0 {
            if let Some(hint) = authorization_hint(exec_status, action) {
                bail!("AuthorizationExecuteWithPrivileges failed (status={exec_status}).\n{hint}");
            }
            bail!("AuthorizationExecuteWithPrivileges failed (status={exec_status})");
        }

        let raw = read_pipe(pipe);
        let combined = String::from_utf8_lossy(&raw).to_string();
        let (exit_code, cleaned) = parse_exit_code(&combined);
        Ok(CommandResult {
            command: cmd_display,
            ok: exit_code == 0,
            stdout: cleaned,
            stderr: String::new(),
            returncode: exit_code,
        })
    }
}

#[cfg(target_os = "macos")]
pub use macos::run_farmctl_authorized;

#[cfg(not(target_os = "macos"))]
pub fn run_farmctl_authorized(
    _farmctl_path: &str,
    _args: &[&str],
    _config_path: &std::path::Path,
    _env_overrides: &[(&str, &str)],
) -> Result<CommandResult> {
    bail!("Privileged execution is only supported on macOS");
}
