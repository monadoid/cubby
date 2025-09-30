use cliclack::confirm;
use service_manager::*;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ffi::OsString, fs, thread};

pub const SERVICE_LABEL_SCREENPIPE: &str = "com.example.cubby.screenpipe";
pub const SCREENPIPE_TCP_ADDR: &str = "localhost:3030";
pub const SCREENPIPE_HTTP_URL: &str = "http://localhost:3030";

// Re-export types for use in main.rs
pub use service_manager::{ServiceLabel, ServiceLevel, ServiceStatus};

pub struct Service {
    pub(crate) label: ServiceLabel,
    pub(crate) level: ServiceLevel,
}

impl Service {
    pub fn new(label: ServiceLabel, level: ServiceLevel) -> Self {
        Self { label, level }
    }

    pub(crate) fn manager(&self) -> Result<Box<dyn ServiceManager>, Box<dyn std::error::Error>> {
        let mut m = <dyn ServiceManager>::native()?;
        m.set_level(self.level)?;
        Ok(m)
    }

    pub fn install_and_start(
        &self,
        program: PathBuf,
        args: Vec<OsString>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contents_override = maybe_custom_launchd_contents(&self.label, &program, &args)?;

        let m = self.manager()?;
        m.install(ServiceInstallCtx {
            label: self.label.clone(),
            program,
            args,
            contents: contents_override,
            username: None,
            working_directory: None,
            environment: None,
            autostart: true,
            disable_restart_on_failure: false,
        })?;
        m.start(ServiceStartCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn uninstall(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx {
            label: self.label.clone(),
        });
        m.uninstall(ServiceUninstallCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx {
            label: self.label.clone(),
        });
        m.start(ServiceStartCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn status(&self) -> Result<ServiceStatus, Box<dyn std::error::Error>> {
        let m = self.manager()?;
        m.status(ServiceStatusCtx {
            label: self.label.clone(),
        })
        .map_err(Into::into)
    }
}

pub struct DualServiceStatus {
    pub screenpipe_running: bool,
    pub cloudflared_running: bool,
}

impl DualServiceStatus {
    pub fn both_running(&self) -> bool {
        self.screenpipe_running && self.cloudflared_running
    }
}

fn check_screenpipe_http_health() -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};

    let mut last_error = None;
    let host_header = SCREENPIPE_TCP_ADDR;
    let request =
        format!("GET / HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\n\r\n").into_bytes();

    for sock in SCREENPIPE_TCP_ADDR
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
    {
        match TcpStream::connect_timeout(&sock, Duration::from_secs(3)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                if let Err(err) = stream.write_all(&request) {
                    last_error = Some(format!("TCP write failed for {sock}: {err}",));
                    continue;
                }

                let mut buf = [0u8; 64];
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let head = String::from_utf8_lossy(&buf[..n]);
                        if head.starts_with("HTTP/1.1") || head.starts_with("HTTP/1.0") {
                            return Ok(());
                        }
                        last_error = Some(format!("unexpected response from {sock}: {head}",));
                    }
                    Ok(_) => {
                        last_error = Some(format!("empty response from {sock}",));
                    }
                    Err(err) => {
                        last_error = Some(format!("TCP read failed for {sock}: {err}",));
                    }
                }
            }
            Err(err) => {
                last_error = Some(format!("TCP connect failed for {sock}: {err}"));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "No socket addresses resolved".to_string()))
}

pub fn status_both(cloudflared_path: Option<PathBuf>) -> DualServiceStatus {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse().unwrap();
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    let screenpipe_running = matches!(svc_screen.status(), Ok(ServiceStatus::Running));

    // Check cloudflared service status using its native command
    let cloudflared_running = if let Some(cloudflared) = cloudflared_path {
        Command::new(cloudflared)
            .args(&["service", "status"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        // Try to find cloudflared in PATH
        Command::new("cloudflared")
            .args(&["service", "status"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    };

    DualServiceStatus {
        screenpipe_running,
        cloudflared_running,
    }
}

pub fn screenpipe_diagnostics() -> Option<String> {
    let mut sections = Vec::new();

    // Check if server is actually responding via HTTP
    match check_screenpipe_http_health() {
        Ok(()) => {
            sections.push("Screenpipe HTTP health check: ✅ responding".to_string());
        }
        Err(err) => {
            sections.push(format!("Screenpipe HTTP health check: ❌ {}", err));
        }
    }

    if let Some((stdout_path, stderr_path)) = screenpipe_log_paths() {
        if let Some(tail) = tail_file(&stdout_path, 50) {
            sections.push(format!(
                "--- screenpipe stdout ({}) ---\n{}",
                stdout_path.display(),
                tail
            ));
        }
        if let Some(tail) = tail_file(&stderr_path, 50) {
            sections.push(format!(
                "--- screenpipe stderr ({}) ---\n{}",
                stderr_path.display(),
                tail
            ));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

pub fn restart_both(cloudflared_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    // Restart screenpipe service
    let _ = svc_screen.restart();

    // Restart cloudflared service using its native command
    if let Some(cloudflared) = cloudflared_path {
        let _ = Command::new(cloudflared)
            .args(&["service", "restart"])
            .output();
    } else {
        // Try to find cloudflared in PATH
        let _ = Command::new("cloudflared")
            .args(&["service", "restart"])
            .output();
    }

    Ok(())
}

pub fn uninstall_both(cloudflared_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    // Uninstall screenpipe service (ignore errors if service doesn't exist)
    let _ = svc_screen.uninstall();

    // Uninstall cloudflared service using its native command
    if let Some(cloudflared) = cloudflared_path {
        let _ = Command::new(cloudflared)
            .args(&["service", "uninstall"])
            .output();
    } else {
        // Try to find cloudflared in PATH
        let _ = Command::new("cloudflared")
            .args(&["service", "uninstall"])
            .output();
    }

    Ok(())
}

pub fn install_both(
    screenpipe: PathBuf,
    cloudflared: PathBuf,
    tunnel_token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1) install/start screenpipe via your ServiceManager
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);
    svc_screen.install_and_start(screenpipe, vec![])?;

    // 2) cloudflared install via its own CLI.
    if cloudflared_service_installed(&cloudflared)? {
        println!(
            "cloudflared service already installed (detected via `service status`). Skipping."
        );
        return Ok(());
    }

    if let Some(existing) = detect_existing_cloudflared_install(&cloudflared)? {
        println!("cloudflared appears to be already set up: {existing}");
        let overwrite =
            confirm("A cloudflared service is already present. Do you want to overwrite it?")
                .initial_value(false)
                .interact()
                .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

        if !overwrite {
            return Err("cloudflared service already installed; aborting per user choice.".into());
        }

        uninstall_cloudflared_service(&cloudflared)?;
    }

    println!(
        "Executing: {:?} service install <token>",
        cloudflared.display()
    );
    let (code, out, err) = run_and_tee(
        cloudflared.clone(),
        &["service", "install", tunnel_token],
        "cloudflared",
    )?;

    if code != 0 {
        return Err(format_cloudflared_install_error(code, &out, &err).into());
    }

    Ok(())
}

fn cloudflared_service_installed(
    cloudflared: &PathBuf,
) -> Result<bool, Box<dyn std::error::Error>> {
    let status = Command::new(cloudflared)
        .args(["service", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(matches!(status, Ok(s) if s.success()))
}

fn run_and_tee(
    program: PathBuf,
    args: &[&str],
    tag: &str,
) -> Result<(i32, String, String), Box<dyn std::error::Error>> {
    let mut child = Command::new(&program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let tag_out = tag.to_owned();
    let out_handle = thread::spawn(move || {
        let mut buf = String::new();
        for line in BufReader::new(stdout).lines() {
            if let Ok(l) = line {
                println!("[{tag_out}][stdout] {l}");
                buf.push_str(&l);
                buf.push('\n');
            }
        }
        buf
    });

    let tag_err = tag.to_owned();
    let err_handle = thread::spawn(move || {
        let mut buf = String::new();
        for line in BufReader::new(stderr).lines() {
            if let Ok(l) = line {
                eprintln!("[{tag_err}][stderr] {l}");
                buf.push_str(&l);
                buf.push('\n');
            }
        }
        buf
    });

    let status = child.wait()?;
    let out = out_handle.join().unwrap();
    let err = err_handle.join().unwrap();
    let code = status.code().unwrap_or(-1);

    Ok((code, out, err))
}

#[cfg(target_os = "macos")]
fn format_cloudflared_install_error(code: i32, out: &str, err: &str) -> String {
    use std::fs;
    let tail = |p: &str| fs::read_to_string(p).unwrap_or_default();
    let out_log = tail("/Library/Logs/com.cloudflare.cloudflared.out.log");
    let err_log = tail("/Library/Logs/com.cloudflare.cloudflared.err.log");

    format!(
        "cloudflared service install failed (exit {code})\n\
         --- stdout ---\n{out}\n\
         --- stderr ---\n{err}\n\
         --- /Library/Logs/com.cloudflare.cloudflared.out.log ---\n{out_log}\n\
         --- /Library/Logs/com.cloudflare.cloudflared.err.log ---\n{err_log}\n"
    )
}

#[cfg(not(target_os = "macos"))]
fn format_cloudflared_install_error(code: i32, out: &str, err: &str) -> String {
    format!(
        "cloudflared service install failed (exit {code})\n--- stdout ---\n{out}\n--- stderr ---\n{err}\n"
    )
}

fn detect_existing_cloudflared_install(
    cloudflared: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Ok(status) = Command::new(cloudflared)
        .args(["service", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        if status.success() {
            return Ok(Some(
                "`cloudflared service status` reports running".to_string(),
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            let launch_agent =
                Path::new(&home).join("Library/LaunchAgents/com.cloudflare.cloudflared.plist");
            if launch_agent.exists() {
                return Ok(Some(format!(
                    "LaunchAgent plist already exists at {}",
                    launch_agent.display()
                )));
            }
        }
    }

    Ok(None)
}

fn uninstall_cloudflared_service(cloudflared: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Uninstalling existing cloudflared service...");
    let uninstall = Command::new(cloudflared)
        .args(["service", "uninstall"])
        .output()?;

    if !uninstall.status.success() {
        let uninstall_stdout = String::from_utf8_lossy(&uninstall.stdout);
        let uninstall_stderr = String::from_utf8_lossy(&uninstall.stderr);
        return Err(format!(
            "Failed to uninstall existing cloudflared service\n--- stdout ---\n{uninstall_stdout}\n--- stderr ---\n{uninstall_stderr}\n"
        )
        .into());
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            let launch_agent =
                Path::new(&home).join("Library/LaunchAgents/com.cloudflare.cloudflared.plist");
            if launch_agent.exists() {
                println!(
                    "cloudflared uninstall reported success but plist still present at {}",
                    launch_agent.display()
                );
            }
        }
    }

    Ok(())
}

fn tail_file(path: &Path, max_lines: usize) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = contents.lines().rev().take(max_lines).collect();
    if lines.is_empty() {
        None
    } else {
        let mut ordered = lines;
        ordered.reverse();
        Some(ordered.join("\n"))
    }
}

#[cfg(target_os = "macos")]
fn maybe_custom_launchd_contents(
    label: &ServiceLabel,
    program: &Path,
    args: &[OsString],
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if label.to_qualified_name() != SERVICE_LABEL_SCREENPIPE {
        return Ok(None);
    }

    if let Some((stdout_path, stderr_path)) = screenpipe_log_paths() {
        if let Some(parent) = stdout_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = stderr_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let plist =
            build_screenpipe_launchd_plist(label, program, args, &stdout_path, &stderr_path);
        Ok(Some(plist))
    } else {
        Ok(None)
    }
}

#[cfg(not(target_os = "macos"))]
fn maybe_custom_launchd_contents(
    _label: &ServiceLabel,
    _program: &Path,
    _args: &[OsString],
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    Ok(None)
}

#[cfg(target_os = "macos")]
fn build_screenpipe_launchd_plist(
    label: &ServiceLabel,
    program: &Path,
    args: &[OsString],
    stdout_path: &Path,
    stderr_path: &Path,
) -> String {
    let mut program_args = Vec::new();
    program_args.push(program.to_string_lossy().into_owned());
    program_args.extend(args.iter().map(|arg| arg.to_string_lossy().into_owned()));

    let program_arguments_xml: String = program_args
        .into_iter()
        .map(|arg| format!("        <string>{}</string>\n", xml_escape(&arg)))
        .collect();

    let label_str = label.to_qualified_name();

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>Label</key>\n\
    <string>{label}</string>\n\
    <key>ProgramArguments</key>\n\
    <array>\n\
{program_arguments}    </array>\n\
    <key>KeepAlive</key>\n\
    <true/>\n\
    <key>RunAtLoad</key>\n\
    <true/>\n\
    <key>StandardOutPath</key>\n\
    <string>{stdout}</string>\n\
    <key>StandardErrorPath</key>\n\
    <string>{stderr}</string>\n\
</dict>\n\
</plist>\n",
        label = xml_escape(&label_str),
        program_arguments = program_arguments_xml,
        stdout = xml_escape(&stdout_path.to_string_lossy()),
        stderr = xml_escape(&stderr_path.to_string_lossy()),
    )
}

#[cfg(target_os = "macos")]
fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
pub fn screenpipe_log_paths() -> Option<(PathBuf, PathBuf)> {
    let home = env::var_os("HOME")?;
    let base = PathBuf::from(home).join("Library").join("Logs");
    let stdout_path = base.join(format!("{}.out.log", SERVICE_LABEL_SCREENPIPE));
    let stderr_path = base.join(format!("{}.err.log", SERVICE_LABEL_SCREENPIPE));
    Some((stdout_path, stderr_path))
}

#[cfg(not(target_os = "macos"))]
pub fn screenpipe_log_paths() -> Option<(PathBuf, PathBuf)> {
    None
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use serial_test::serial;
    use std::process::Command;
    use std::time::Duration;

    const SERVER_URL: &str = SCREENPIPE_HTTP_URL;

    fn get_services() -> Service {
        let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse().unwrap();
        Service::new(label_screen, ServiceLevel::User)
    }

    #[tokio::test]
    async fn test_cli_help() {
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Give your computer MCP superpowers",
            ));
    }

    #[tokio::test]
    async fn test_cli_version() {
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("--version")
            .assert()
            .success()
            .stdout(predicate::str::contains("cubby"));
    }

    #[tokio::test]
    #[serial]
    async fn test_service_lifecycle() {
        // Ensure service is not running initially
        cleanup_service().await;

        // Test start command
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("start")
            .assert()
            .success()
            .stdout(predicate::str::contains("Service started successfully"));

        // Test status command - should show running
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Running"));

        // Test server is actually responding on port 3030
        tokio::time::timeout(Duration::from_secs(30), test_server_health())
            .await
            .expect("Server health check timed out")
            .expect("Server should be healthy");

        // Test restart command
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("restart");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Service restarted successfully"));

        // Verify service is still running after restart
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("status");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Running"));

        // Test uninstall command
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("uninstall");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Service uninstalled successfully"));

        // Test status command - should show not running
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("status");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Not running"));
    }

    #[tokio::test]
    #[serial]
    async fn test_direct_service_start_and_health() {
        cleanup_service().await;

        // Test starting services directly via start command
        let mut start_cmd = Command::cargo_bin("cubby").unwrap();
        start_cmd.arg("start");
        let _start_result = start_cmd.assert().success();

        // Wait a bit for services to fully start
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Test server health
        match test_server_health().await {
            Ok(_) => println!("Server health check passed"),
            Err(e) => {
                cleanup_service().await;
                panic!("Server health check failed: {}", e);
            }
        }

        // Clean up
        cleanup_service().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_service_installation_edge_cases() {
        cleanup_service().await;

        // Test starting when already installed
        let mut cmd1 = Command::cargo_bin("cubby").unwrap();
        cmd1.arg("start");
        cmd1.assert().success();

        // Starting again should handle gracefully
        let mut cmd2 = Command::cargo_bin("cubby").unwrap();
        cmd2.arg("start");
        // This might fail or succeed depending on implementation, but shouldn't crash
        cmd2.assert().code(predicate::in_iter([0, 1]));

        // Test uninstalling when not installed
        cleanup_service().await;
        let mut cmd3 = Command::cargo_bin("cubby").unwrap();
        cmd3.arg("uninstall");
        // Should handle gracefully
        cmd3.assert().code(predicate::in_iter([0, 1]));
    }

    async fn test_server_health() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();

        // Try to connect to the MCP server
        let response = client
            .get(SERVER_URL)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("Server responded with status: {}", resp.status());
                Ok(())
            }
            Err(e) => {
                println!("Server health check failed: {}", e);
                Err(e.into())
            }
        }
    }

    async fn cleanup_service() {
        // Use our service manager to clean up both services
        let _ = uninstall_both(None);

        // Kill any processes using port 3030
        let _ = Command::new("bash")
            .args(&["-c", "lsof -ti:3030 | xargs kill -9 2>/dev/null || true"])
            .output();
    }
}
