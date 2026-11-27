//! 结束和启动 Steam，并在启动后校验 SLSsteam 是否真的注入。

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};

use crate::steam::env;

const STOP_TIMEOUT: Duration = Duration::from_secs(12);
const INJECT_TIMEOUT: Duration = Duration::from_secs(25);

/// `Some(true)` 已注入，`Some(false)` 未注入，`None` 无法判断。
pub fn injected() -> Option<bool> {
    let processes = env::find_steam_processes();
    if processes.is_empty() {
        return Some(false);
    }
    if processes.iter().any(|process| process.injected == Some(true)) {
        return Some(true);
    }
    if processes
        .iter()
        .all(|process| process.injected == Some(false))
    {
        return Some(false);
    }
    None
}

/// 结束 Steam，用带注入的入口重新启动，并等待注入生效。
pub fn restart() -> Result<String, String> {
    stop()?;
    start()?;
    let deadline = Instant::now() + INJECT_TIMEOUT;
    while Instant::now() < deadline {
        if injected() == Some(true) {
            return Ok("Steam 已重新启动，SLSsteam 注入成功。".to_owned());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err("Steam 已启动，但 25 秒内没有检测到 SLSsteam 注入。请检查启动入口和 SLSsteam.so。".to_owned())
}

fn stop() -> Result<(), String> {
    let processes = env::find_steam_processes();
    if processes.is_empty() {
        return Ok(());
    }
    for process in &processes {
        if let Some(pid) = Pid::from_raw(process.pid) {
            let _ = kill_process(pid, Signal::TERM);
        }
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        if env::find_steam_processes().is_empty() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    for process in env::find_steam_processes() {
        if let Some(pid) = Pid::from_raw(process.pid) {
            let _ = kill_process(pid, Signal::KILL);
        }
    }
    thread::sleep(Duration::from_millis(500));
    if env::find_steam_processes().is_empty() {
        Ok(())
    } else {
        Err("无法结束全部 Steam 进程，请手动退出 Steam 后再试。".to_owned())
    }
}

fn start() -> Result<(), String> {
    let (program, needs_audit) = launcher();
    let mut command = Command::new(&program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("LD_PRELOAD")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_AUDIT");
    if needs_audit {
        if let Some(library) = env::sls_so_path() {
            command.env("LD_AUDIT", library);
        }
    }
    command
        .spawn()
        .map_err(|error| format!("无法启动 {}：{error}", program.display()))?;
    Ok(())
}

/// 返回启动程序和是否需要由我们补 `LD_AUDIT`。
fn launcher() -> (PathBuf, bool) {
    let Some(home) = std::env::home_dir() else {
        return (PathBuf::from("steam"), env::sls_so_path().is_some());
    };
    let wrapper = home.join(".local/bin/steam");
    if wrapper.is_file() {
        let has_audit = fs::read_to_string(&wrapper)
            .map(|text| text.contains("LD_AUDIT"))
            .unwrap_or(false);
        return (wrapper, !has_audit);
    }
    (PathBuf::from("steam"), env::sls_so_path().is_some())
}
