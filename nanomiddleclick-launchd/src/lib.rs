use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LABEL: &str = "co.myrt.nanomiddleclick";
const PLIST_FILENAME: &str = "co.myrt.nanomiddleclick.plist";
const STDOUT_LOG_FILENAME: &str = "nanomiddleclick.stdout.log";
const STDERR_LOG_FILENAME: &str = "nanomiddleclick.stderr.log";

pub fn install(executable_path: &Path) -> Result<PathBuf, String> {
    let paths = agent_paths()?;
    fs::create_dir_all(&paths.launch_agents_dir).map_err(|error| {
        format!("failed to create {}: {error}", paths.launch_agents_dir.display())
    })?;
    fs::create_dir_all(&paths.logs_dir).map_err(|error| {
        format!("failed to create {}: {error}", paths.logs_dir.display())
    })?;

    let plist = render_plist(
        executable_path,
        &paths.stdout_log_path,
        &paths.stderr_log_path,
    );
    fs::write(&paths.plist_path, plist).map_err(|error| {
        format!("failed to write {}: {error}", paths.plist_path.display())
    })?;

    unload_agent(&paths.plist_path, false)?;
    load_agent(&paths.plist_path)?;
    Ok(paths.plist_path)
}

pub fn uninstall() -> Result<PathBuf, String> {
    let paths = agent_paths()?;
    unload_agent(&paths.plist_path, false)?;

    if paths.plist_path.exists() {
        fs::remove_file(&paths.plist_path).map_err(|error| {
            format!("failed to remove {}: {error}", paths.plist_path.display())
        })?;
    }

    Ok(paths.plist_path)
}

pub fn render_plist(
    executable_path: &Path,
    stdout_log_path: &Path,
    stderr_log_path: &Path,
) -> String {
    let executable = escape_xml(&executable_path.display().to_string());
    let stdout = escape_xml(&stdout_log_path.display().to_string());
    let stderr = escape_xml(&stderr_log_path.display().to_string());

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\"\n",
            "    \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n",
            "<dict>\n",
            "  <key>Label</key>\n",
            "  <string>{LABEL}</string>\n",
            "\n",
            "  <key>ProgramArguments</key>\n",
            "  <array>\n",
            "    <string>{executable}</string>\n",
            "  </array>\n",
            "\n",
            "  <key>RunAtLoad</key>\n",
            "  <true />\n",
            "\n",
            "  <key>KeepAlive</key>\n",
            "  <dict>\n",
            "    <key>SuccessfulExit</key>\n",
            "    <false />\n",
            "  </dict>\n",
            "\n",
            "  <key>StandardOutPath</key>\n",
            "  <string>{stdout}</string>\n",
            "\n",
            "  <key>StandardErrorPath</key>\n",
            "  <string>{stderr}</string>\n",
            "</dict>\n",
            "</plist>\n"
        ),
        LABEL = LABEL,
        executable = executable,
        stdout = stdout,
        stderr = stderr,
    )
}

fn agent_paths() -> Result<AgentPaths, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_owned())?;
    let library_dir = home.join("Library");
    let launch_agents_dir = library_dir.join("LaunchAgents");
    let logs_dir = library_dir.join("Logs");

    Ok(AgentPaths {
        plist_path: launch_agents_dir.join(PLIST_FILENAME),
        stdout_log_path: logs_dir.join(STDOUT_LOG_FILENAME),
        stderr_log_path: logs_dir.join(STDERR_LOG_FILENAME),
        launch_agents_dir,
        logs_dir,
    })
}

fn load_agent(plist_path: &Path) -> Result<(), String> {
    run_launchctl([OsStr::new("load"), plist_path.as_os_str()], "load")
}

fn unload_agent(plist_path: &Path, required: bool) -> Result<(), String> {
    if !required && !plist_path.exists() {
        return Ok(());
    }

    match run_launchctl([OsStr::new("unload"), plist_path.as_os_str()], "unload") {
        Ok(()) => Ok(()),
        Err(_) if !required => Ok(()),
        Err(error) => Err(error),
    }
}

fn run_launchctl<const N: usize>(
    args: [&OsStr; N],
    action: &str,
) -> Result<(), String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run launchctl {action}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };

    Err(format!("launchctl {action} failed: {details}"))
}

fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

struct AgentPaths {
    plist_path: PathBuf,
    stdout_log_path: PathBuf,
    stderr_log_path: PathBuf,
    launch_agents_dir: PathBuf,
    logs_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::render_plist;
    use std::path::Path;

    #[test]
    fn render_plist_uses_expected_label_and_paths() {
        let plist = render_plist(
            Path::new("/tmp/nanomiddleclick"),
            Path::new("/tmp/stdout.log"),
            Path::new("/tmp/stderr.log"),
        );

        assert!(plist.contains("<string>co.myrt.nanomiddleclick</string>"));
        assert!(plist.contains("<string>/tmp/nanomiddleclick</string>"));
        assert!(plist.contains("<string>/tmp/stdout.log</string>"));
        assert!(plist.contains("<string>/tmp/stderr.log</string>"));
    }

    #[test]
    fn render_plist_escapes_xml_sensitive_characters() {
        let plist = render_plist(
            Path::new("/tmp/a&b<nanomiddleclick>"),
            Path::new("/tmp/stdout\".log"),
            Path::new("/tmp/stderr'.log"),
        );

        assert!(plist.contains("/tmp/a&amp;b&lt;nanomiddleclick&gt;"));
        assert!(plist.contains("/tmp/stdout&quot;.log"));
        assert!(plist.contains("/tmp/stderr&apos;.log"));
    }
}
