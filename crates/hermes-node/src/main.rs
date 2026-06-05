use std::{
    env,
    fs,
    io::{self, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::Duration,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use hermes_core::{
    consensus::Yc3Input,
    config::HermesConfig,
    runtime::DeploymentPlanner,
    Yc3Engine, Yc3Parameters,
};

#[derive(Debug, Parser)]
#[command(name = "hermes-node")]
#[command(about = "Hermes validator runtime and planning CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ValidateConfig { path: PathBuf },
    Plan { path: PathBuf },
    Control,
    Serve {
        path: PathBuf,
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long, default_value_t = 60)]
        interval_seconds: u64,
        #[arg(long, default_value_t = false)]
        once: bool,
    },
    ScoreSnapshot {
        path: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        pretty: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Control => {
            run_control_plane()?;
        }
        Command::ValidateConfig { path } => {
            let config = load_config(&path)?;
            config.validate()?;
            println!("validated {}", path.display());
        }
        Command::Plan { path } => {
            let config = load_config(&path)?;
            let plan = DeploymentPlanner::build(&config)?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Command::Serve {
            path,
            snapshot,
            interval_seconds,
            once,
        } => {
            run_serve(path, snapshot, interval_seconds, once)?;
        }
        Command::ScoreSnapshot {
            path,
            config,
            pretty,
        } => {
            let input = load_snapshot(&path)?;
            let params = if let Some(config_path) = config {
                let config = load_config(&config_path)?;
                Yc3Parameters {
                    kappa: config.consensus.kappa,
                    bonds_penalty: config.consensus.bonds_penalty,
                    ema_alpha: config.consensus.ema_alpha,
                    validator_emission_ratio: config.consensus.validator_emission_ratio,
                }
            } else {
                Yc3Parameters::default()
            };

            let report = Yc3Engine::new(params).evaluate(&input)?;
            if pretty {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", serde_json::to_string(&report)?);
            }
        }
    }

    Ok(())
}

fn load_config(path: &PathBuf) -> Result<HermesConfig> {
    let raw = fs::read_to_string(path)?;
    Ok(HermesConfig::from_json_str(&raw)?)
}

fn load_snapshot(path: &PathBuf) -> Result<Yc3Input> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn run_serve(
    path: PathBuf,
    snapshot: Option<PathBuf>,
    interval_seconds: u64,
    once: bool,
) -> Result<()> {
    let config = load_config(&path)?;
    config.validate()?;

    loop {
        let plan = DeploymentPlanner::build(&config)?;
        println!("{}", serde_json::to_string(&plan)?);

        if let Some(snapshot_path) = snapshot.as_ref() {
            let params = Yc3Parameters {
                kappa: config.consensus.kappa,
                bonds_penalty: config.consensus.bonds_penalty,
                ema_alpha: config.consensus.ema_alpha,
                validator_emission_ratio: config.consensus.validator_emission_ratio,
            };
            let report = Yc3Engine::new(params).evaluate(&load_snapshot(snapshot_path)?)?;
            println!(
                "processed snapshot with {} validators and {} miners",
                report.validators.len(),
                report.miners.len()
            );
        } else {
            println!("validated runtime configuration for {}", config.cluster.network.public_endpoint);
        }

        if once {
            break;
        }

        thread::sleep(Duration::from_secs(interval_seconds.max(5)));
    }

    Ok(())
}

fn run_control_plane() -> Result<()> {
    ensure_dirs()?;

    loop {
        print_header("Hermes Operator CLI");
        println!("Root: {}", workspace_root().display());
        println!("Gateway: http://127.0.0.1:{}", gateway_port());
        println!();
        println!("1) Start full stack");
        println!("2) Stop full stack");
        println!("3) Restart full stack");
        println!("4) Show stack status");
        println!("5) Open gateway in browser");
        println!("6) Validate config");
        println!("7) Plan deployment");
        println!("8) Score snapshot");
        println!("9) Reliability and validation");
        println!("10) Log inspection");
        println!("0) Exit");

        let choice = prompt("\nChoice", None)?;
        match choice.trim() {
            "1" => {
                start_stack()?;
                pause()?;
            }
            "2" => {
                stop_stack()?;
                pause()?;
            }
            "3" => {
                stop_stack()?;
                start_stack()?;
                pause()?;
            }
            "4" => {
                show_status()?;
                pause()?;
            }
            "5" => {
                open_gateway()?;
                pause()?;
            }
            "6" => {
                run_sync(
                    "Validate config",
                    &[
                        current_exe()?,
                        "validate-config".to_string(),
                        default_config_path().display().to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "7" => {
                run_sync(
                    "Plan deployment",
                    &[
                        current_exe()?,
                        "plan".to_string(),
                        default_config_path().display().to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "8" => {
                run_sync(
                    "Score snapshot",
                    &[
                        current_exe()?,
                        "score-snapshot".to_string(),
                        default_snapshot_path().display().to_string(),
                        "--config".to_string(),
                        default_config_path().display().to_string(),
                        "--pretty".to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "9" => {
                reliability_menu()?;
            }
            "10" => {
                logs_menu()?;
            }
            "0" => return Ok(()),
            other => {
                println!("Unknown option: {}", other);
                pause()?;
            }
        }
    }
}

fn reliability_menu() -> Result<()> {
    loop {
        print_header("Reliability and Validation");
        println!("1) cargo test --workspace");
        println!("2) Validate config");
        println!("3) Plan deployment");
        println!("4) Score snapshot");
        println!("5) Gateway typecheck");
        println!("6) Gateway test");
        println!("7) Helm template render");
        println!("0) Back");

        let choice = prompt("\nChoice", None)?;
        match choice.trim() {
            "1" => {
                run_shell("cargo test --workspace", &workspace_root())?;
                pause()?;
            }
            "2" => {
                run_sync(
                    "Validate config",
                    &[
                        current_exe()?,
                        "validate-config".to_string(),
                        default_config_path().display().to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "3" => {
                run_sync(
                    "Plan deployment",
                    &[
                        current_exe()?,
                        "plan".to_string(),
                        default_config_path().display().to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "4" => {
                run_sync(
                    "Score snapshot",
                    &[
                        current_exe()?,
                        "score-snapshot".to_string(),
                        default_snapshot_path().display().to_string(),
                        "--config".to_string(),
                        default_config_path().display().to_string(),
                        "--pretty".to_string(),
                    ],
                    &workspace_root(),
                )?;
                pause()?;
            }
            "5" => {
                run_shell("npm run typecheck", &gateway_dir())?;
                pause()?;
            }
            "6" => {
                run_shell("npm test", &gateway_dir())?;
                pause()?;
            }
            "7" => {
                run_shell("helm template hermes ./deploy/helm/hermes", &workspace_root())?;
                pause()?;
            }
            "0" => return Ok(()),
            other => {
                println!("Unknown option: {}", other);
                pause()?;
            }
        }
    }
}

fn logs_menu() -> Result<()> {
    loop {
        print_header("Log Inspection");
        println!("1) Tail node.out");
        println!("2) Tail node.err");
        println!("3) Tail gateway.out");
        println!("4) Tail gateway.err");
        println!("0) Back");

        let choice = prompt("\nChoice", None)?;
        match choice.trim() {
            "1" => {
                tail_file(&node_out_log())?;
                pause()?;
            }
            "2" => {
                tail_file(&node_err_log())?;
                pause()?;
            }
            "3" => {
                tail_file(&gateway_out_log())?;
                pause()?;
            }
            "4" => {
                tail_file(&gateway_err_log())?;
                pause()?;
            }
            "0" => return Ok(()),
            other => {
                println!("Unknown option: {}", other);
                pause()?;
            }
        }
    }
}

fn print_header(title: &str) {
    println!("====================================================");
    println!("  {}", title);
    println!("====================================================");
}

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(value) => print!("{} [{}]: ", label, value),
        None => print!("{}: ", label),
    }
    io::stdout().flush()?;

    let mut buffer = String::new();
    if io::stdin().read_line(&mut buffer)? == 0 {
        return Ok("0".to_string());
    }

    let value = buffer.trim().to_string();
    if value.is_empty() {
        Ok(default.unwrap_or_default().to_string())
    } else {
        Ok(value)
    }
}

fn pause() -> Result<()> {
    print!("\nPress Enter to continue...");
    io::stdout().flush()?;
    let mut buffer = String::new();
    let _ = io::stdin().read_line(&mut buffer)?;
    Ok(())
}

fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(run_dir())?;
    fs::create_dir_all(logs_dir())?;
    Ok(())
}

fn start_stack() -> Result<()> {
    ensure_dirs()?;

    spawn_managed_process(
        "node runtime",
        &node_pid_file(),
        &[current_exe()?, "serve".to_string(), default_config_path().display().to_string()],
        &workspace_root(),
        &node_out_log(),
        &node_err_log(),
    )?;

    spawn_gateway_process()?;
    Ok(())
}

fn stop_stack() -> Result<()> {
    stop_by_pid_file("gateway", &gateway_pid_file())?;
    stop_by_pid_file("node runtime", &node_pid_file())?;
    Ok(())
}

fn show_status() -> Result<()> {
    let node_pid = read_pid(&node_pid_file())?;
    let gateway_pid = read_pid(&gateway_pid_file())?;
    let node_running = node_pid.is_some_and(process_running);
    let gateway_running = gateway_pid.is_some_and(process_running);
    let gateway_healthy = http_port_reachable(gateway_port());

    print_header("Hermes Stack Status");
    println!("Root:            {}", workspace_root().display());
    println!("Gateway URL:     http://127.0.0.1:{}", gateway_port());
    println!(
        "Node runtime:    {}",
        if node_running {
            format!("running ({})", node_pid.unwrap_or_default())
        } else {
            "not running".to_string()
        }
    );
    println!(
        "Gateway:         {}",
        if gateway_running {
            format!("running ({})", gateway_pid.unwrap_or_default())
        } else {
            "not running".to_string()
        }
    );
    println!("Gateway health:  {}", if gateway_healthy { "OK" } else { "DOWN" });
    Ok(())
}

fn open_gateway() -> Result<()> {
    let url = format!("http://127.0.0.1:{}", gateway_port());
    #[cfg(target_os = "windows")]
    {
        ProcessCommand::new("cmd")
            .args(["/C", "start", "", &url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        ProcessCommand::new("open")
            .arg(&url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        ProcessCommand::new("xdg-open")
            .arg(&url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }

    println!("Opened {} in default browser.", url);
    Ok(())
}

fn spawn_gateway_process() -> Result<()> {
    let (command, args): (String, Vec<String>) = if cfg!(target_os = "windows") {
        (
            "cmd".to_string(),
            vec!["/C".to_string(), "bun run dev".to_string()],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-lc".to_string(), "bun run dev".to_string()],
        )
    };

    let mut values = vec![command];
    values.extend(args);
    spawn_managed_process(
        "gateway",
        &gateway_pid_file(),
        &values,
        &gateway_dir(),
        &gateway_out_log(),
        &gateway_err_log(),
    )
}

fn spawn_managed_process(
    label: &str,
    pid_file: &Path,
    values: &[String],
    cwd: &Path,
    out_log: &Path,
    err_log: &Path,
) -> Result<()> {
    let existing = read_pid(pid_file)?;
    if let Some(pid) = existing {
        if process_running(pid) {
            println!("{}: already running ({})", label, pid);
            return Ok(());
        }
        remove_pid_file(pid_file)?;
    }

    if values.is_empty() {
        return Ok(());
    }

    let out = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_log)?;
    let err = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(err_log)?;

    let mut command = ProcessCommand::new(&values[0]);
    if values.len() > 1 {
        command.args(&values[1..]);
    }

    let child = command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()?;

    fs::write(pid_file, child.id().to_string())?;
    println!("{}: started ({})", label, child.id());
    Ok(())
}

fn stop_by_pid_file(label: &str, pid_file: &Path) -> Result<()> {
    let pid = read_pid(pid_file)?;
    match pid {
        None => {
            println!("{}: not running", label);
            remove_pid_file(pid_file)?;
        }
        Some(id) => {
            if !process_running(id) {
                println!("{}: stale pid ({}) removed", label, id);
                remove_pid_file(pid_file)?;
                return Ok(());
            }

            #[cfg(target_os = "windows")]
            {
                let _ = ProcessCommand::new("taskkill")
                    .args(["/PID", &id.to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = ProcessCommand::new("kill")
                    .args(["-TERM", &id.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }

            remove_pid_file(pid_file)?;
            println!("{}: stopped ({})", label, id);
        }
    }

    Ok(())
}

fn process_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(status) = ProcessCommand::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            let output = String::from_utf8_lossy(&status.stdout);
            return output.contains(&pid.to_string());
        }
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        ProcessCommand::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|value| value.success())
            .unwrap_or(false)
    }
}

fn read_pid(pid_file: &Path) -> Result<Option<u32>> {
    if !pid_file.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(pid_file)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed.parse::<u32>().ok();
    Ok(parsed)
}

fn remove_pid_file(pid_file: &Path) -> Result<()> {
    if pid_file.exists() {
        fs::remove_file(pid_file)?;
    }
    Ok(())
}

fn http_port_reachable(port: u16) -> bool {
    let target = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&target, Duration::from_secs(2)).is_ok()
}

fn tail_file(path: &Path) -> Result<()> {
    if !path.exists() {
        println!("File does not exist: {}", path.display());
        return Ok(());
    }

    let data = fs::read_to_string(path)?;
    let lines: Vec<&str> = data.lines().collect();
    let start = lines.len().saturating_sub(80);
    for line in &lines[start..] {
        println!("{}", line);
    }
    Ok(())
}

fn run_sync(label: &str, values: &[String], cwd: &Path) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    println!("\nRunning {}: {}\n", label, values.join(" "));

    let mut command = ProcessCommand::new(&values[0]);
    if values.len() > 1 {
        command.args(&values[1..]);
    }

    let status = command.current_dir(cwd).status()?;
    println!("\n{} exit code: {}", label, status.code().unwrap_or(1));
    Ok(())
}

fn run_shell(command: &str, cwd: &Path) -> Result<()> {
    println!("\nRunning: {}\n", command);
    let status = if cfg!(target_os = "windows") {
        ProcessCommand::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .status()?
    } else {
        ProcessCommand::new("sh")
            .args(["-lc", command])
            .current_dir(cwd)
            .status()?
    };
    println!("\nCommand exit code: {}", status.code().unwrap_or(1));
    Ok(())
}

fn current_exe() -> Result<String> {
    Ok(env::current_exe()?.display().to_string())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn gateway_dir() -> PathBuf {
    workspace_root().join("gateway")
}

fn run_dir() -> PathBuf {
    workspace_root().join("run")
}

fn logs_dir() -> PathBuf {
    workspace_root().join("logs")
}

fn node_pid_file() -> PathBuf {
    run_dir().join("node.pid")
}

fn gateway_pid_file() -> PathBuf {
    run_dir().join("gateway.pid")
}

fn node_out_log() -> PathBuf {
    logs_dir().join("node.out")
}

fn node_err_log() -> PathBuf {
    logs_dir().join("node.err")
}

fn gateway_out_log() -> PathBuf {
    logs_dir().join("gateway.out")
}

fn gateway_err_log() -> PathBuf {
    logs_dir().join("gateway.err")
}

fn default_config_path() -> PathBuf {
    workspace_root().join("configs").join("hermes.example.json")
}

fn default_snapshot_path() -> PathBuf {
    workspace_root().join("fixtures").join("epoch-snapshot.json")
}

fn gateway_port() -> u16 {
    env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000)
}