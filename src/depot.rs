use std::env;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{Pid, Signal, kill_process_group};
use serde::Deserialize;

const MAX_LINE: u64 = 1024 * 1024;

#[derive(Clone, Copy)]
pub struct Depot {
    pub id: u32,
    pub manifest: Option<u64>,
}

pub struct Plan {
    pub app_id: u32,
    pub depots: Vec<Depot>,
}

pub enum Request {
    Inspect(PathBuf),
    Fetch {
        app_id: u32,
        depot: Depot,
        library: PathBuf,
    },
}

pub enum Outcome {
    Inspected(Plan),
    Downloaded,
    Cancelled,
}

#[derive(Clone, Default)]
pub struct Progress {
    pub output: Option<PathBuf>,
    pub detail: String,
    pub fraction: Option<f32>,
}

pub struct Job {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    worker: Option<JoinHandle<Result<Outcome, String>>>,
}

impl Job {
    pub fn start(request: Request) -> io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(Progress::default()));
        let worker_cancel = cancel.clone();
        let worker_progress = progress.clone();
        let worker = thread::Builder::new()
            .name("steam-depot".into())
            .spawn(move || execute(request, &worker_cancel, &worker_progress))?;
        Ok(Self {
            cancel,
            progress,
            worker: Some(worker),
        })
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }

    pub fn try_finish(&mut self) -> Option<Result<Outcome, String>> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        Some(
            self.worker
                .take()?
                .join()
                .unwrap_or_else(|_| Err("后台任务意外结束。".into())),
        )
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn execute(
    request: Request,
    cancel: &AtomicBool,
    progress: &Mutex<Progress>,
) -> Result<Outcome, String> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(Outcome::Cancelled);
    }
    let working_dir = tempfile::tempdir().map_err(|err| format!("无法创建临时目录（{err}）。"))?;
    match request {
        Request::Inspect(path) => {
            let path = path
                .canonicalize()
                .map_err(|err| format!("无法打开清单（{err}）。"))?;
            let metadata = path
                .metadata()
                .map_err(|err| format!("无法读取清单（{err}）。"))?;
            if !metadata.is_file() || metadata.len() > MAX_LINE {
                return Err("请选择不超过 1 MiB 的 .lua 或 .st 清单文件。".into());
            }
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("lua" | "st")
            ) {
                return Err("请选择 .lua 或 .st 清单文件。".into());
            }
            let mut command = tool("GAMEHAVEN_SMD", "smd")?;
            command
                .current_dir(working_dir.path())
                .arg("--data-dir")
                .arg(working_dir.path())
                .args(["download", "--list", "--json"])
                .arg(path);
            let mut plan = None;
            let mut detail = String::new();
            let result = run_cli(
                command,
                "smd",
                cancel,
                Some(Duration::from_secs(45)),
                |line| {
                    if line.starts_with('{') {
                        plan = Some(parse_plan(line));
                    } else if !line.is_empty() {
                        detail = line.chars().take(240).collect();
                    }
                },
            );
            match result {
                Ok(false) => Ok(Outcome::Cancelled),
                Ok(true) => plan
                    .ok_or_else(|| "smd 没有返回清单计划。".to_owned())?
                    .map(Outcome::Inspected),
                Err(error) if detail.is_empty() => Err(error),
                Err(error) => Err(format!("{error} {detail}")),
            }
        }
        Request::Fetch {
            app_id,
            depot,
            library,
        } => {
            let mut command = tool("GAMEHAVEN_DEPOT_DOWNLOADER", "DepotDownloader")?;
            let library = library
                .canonicalize()
                .map_err(|err| format!("无法打开目标库（{err}）。"))?;
            let output = tempfile::Builder::new()
                .prefix(&format!("steam-{app_id}-depot-{}-", depot.id))
                .tempdir_in(library)
                .map_err(|err| format!("无法创建下载目录（{err}）。"))?
                .keep();
            progress.lock().unwrap().output = Some(output.clone());
            command
                .current_dir(working_dir.path())
                .env("LC_ALL", "C")
                .arg("-app")
                .arg(app_id.to_string())
                .arg("-depot")
                .arg(depot.id.to_string())
                .arg("-dir")
                .arg(output);
            if let Some(manifest_id) = depot.manifest {
                command.arg("-manifest").arg(manifest_id.to_string());
            }
            let mut completed = false;
            if !run_cli(command, "DepotDownloader", cancel, None, |line| {
                if line.starts_with("Total downloaded: ") {
                    completed = line.ends_with(" from 1 depots");
                }
                if let Some(fraction) = download_fraction(line) {
                    let mut state = progress.lock().unwrap();
                    state.fraction = Some(state.fraction.unwrap_or(0.0).max(fraction));
                } else if !line.is_empty() {
                    progress.lock().unwrap().detail = line.chars().take(240).collect();
                }
            })? {
                return Ok(Outcome::Cancelled);
            }
            if !completed {
                return Err("DepotDownloader 退出了，但没有报告所选 Depot 下载完成。".into());
            }
            Ok(Outcome::Downloaded)
        }
    }
}

fn tool(variable: &str, default: &str) -> Result<Command, String> {
    match env::var_os(variable) {
        Some(path) if Path::new(&path).is_absolute() => Ok(Command::new(path)),
        Some(_) => Err(format!("{variable} 需要指向可执行文件的绝对路径。")),
        None => Ok(Command::new(default)),
    }
}

struct ChildProcess(Child);

impl Drop for ChildProcess {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = kill_process_group(Pid::from_child(&self.0), Signal::KILL);
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

// Returns false when cancellation stops the process. The caller keeps the last CLI message.
fn run_cli(
    mut command: Command,
    name: &str,
    cancel: &AtomicBool,
    deadline: Option<Duration>,
    mut on_line: impl FnMut(&str),
) -> Result<bool, String> {
    let describe_error = |err| format!("{name} 进程错误（{err}）。");
    let (reader, writer) = io::pipe().map_err(describe_error)?;
    fcntl_setfl(
        &reader,
        fcntl_getfl(&reader)
            .map_err(io::Error::from)
            .map_err(describe_error)?
            | OFlags::NONBLOCK,
    )
    .map_err(io::Error::from)
    .map_err(describe_error)?;
    let mut child = ChildProcess(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer.try_clone().map_err(describe_error)?))
            .stderr(Stdio::from(writer))
            .process_group(0)
            .spawn()
            .map_err(|err| match err.kind() {
                io::ErrorKind::NotFound => format!("找不到 {name}。请检查 PATH 或配置工具路径。"),
                _ => describe_error(err),
            })?,
    );
    drop(command);

    let started = Instant::now();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        let exit = child.0.try_wait().map_err(describe_error)?;
        if exit.is_none() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            if deadline.is_some_and(|limit| started.elapsed() >= limit) {
                return Err(format!("{name} 响应超时。"));
            }
        }

        let read = (&mut reader)
            .take(MAX_LINE + 1 - line.len() as u64)
            .read_until(b'\n', &mut line);
        if line.len() as u64 > MAX_LINE {
            return Err(format!("{name} 输出的单行超过 1 MiB。"));
        }
        let blocked = match read {
            Ok(0) => true,
            Ok(_) => false,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => true,
            Err(err) => return Err(describe_error(err)),
        };
        if line.ends_with(b"\n") || (blocked && exit.is_some() && !line.is_empty()) {
            on_line(String::from_utf8_lossy(&line).trim());
            line.clear();
        }

        if blocked {
            if let Some(status) = exit {
                return if status.success() {
                    Ok(true)
                } else {
                    Err(format!("{name} 退出失败（{status}）。"))
                };
            }
            thread::sleep(Duration::from_millis(80));
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliPlan {
    #[serde(rename = "type")]
    kind: String,
    app_id: String,
    depots: Vec<CliDepot>,
    total_depots: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliDepot {
    depot_id: String,
    manifest_id: Option<String>,
}

fn parse_plan(line: &str) -> Result<Plan, String> {
    let plan: CliPlan = serde_json::from_str(line).map_err(|_| "无法读取 smd 返回的清单计划。")?;
    let app_id = plan
        .app_id
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("清单缺少有效的 AppID。")?;
    if plan.kind != "plan" || plan.total_depots != plan.depots.len() || plan.depots.is_empty() {
        return Err("清单缺少可下载的 Depot。".into());
    }
    let mut depots = Vec::with_capacity(plan.depots.len());
    for depot in plan.depots {
        let id = depot
            .depot_id
            .parse::<u32>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or("清单包含无效的 Depot ID。")?;
        let manifest = depot
            .manifest_id
            .map(|value| {
                value
                    .parse::<u64>()
                    .ok()
                    .filter(|id| *id > 0 && *id != u64::MAX)
                    .ok_or("清单包含无效的 Manifest ID。")
            })
            .transpose()?;
        depots.push(Depot { id, manifest });
    }
    depots.sort_unstable_by_key(|depot| depot.id);
    if depots.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err("清单包含重复的 Depot ID。".into());
    }
    Ok(Plan { app_id, depots })
}

fn download_fraction(line: &str) -> Option<f32> {
    let (percent, _) = line.split_once("% ")?;
    let percent = percent.trim().replace(',', ".").parse::<f32>().ok()?;
    (0.0..=100.0).contains(&percent).then_some(percent / 100.0)
}

#[cfg(test)]
mod tests {
    use super::{download_fraction, parse_plan};

    #[test]
    fn manifest_ids_keep_full_integer_precision() {
        let plan = parse_plan(r#"{"type":"plan","appId":"232250","totalDepots":1,"depots":[{"depotId":"232257","manifestId":"4797708003880603728"}]}"#).unwrap();
        assert_eq!(plan.app_id, 232250);
        assert_eq!(plan.depots[0].id, 232257);
        assert_eq!(plan.depots[0].manifest, Some(4_797_708_003_880_603_728));
    }

    #[test]
    fn parses_only_valid_download_progress() {
        assert_eq!(download_fraction("  12.50% fixture.bin"), Some(0.125));
        assert_eq!(download_fraction("100,00% fixture.bin"), Some(1.0));
        assert_eq!(download_fraction("101% fixture.bin"), None);
        assert_eq!(download_fraction("Total downloaded: 7 bytes"), None);
    }
}
