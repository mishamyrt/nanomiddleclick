use std::env;

use crate::settings::DEFAULTS_DOMAIN;
use lunchd::LaunchAgent;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum LaunchAgentError {
    #[error("build failed")]
    BuildFailed(#[from] lunchd::LaunchAgentBuilderError),

    #[error("launchd action failed")]
    AgentActionFailed(#[from] lunchd::AgentError),

    #[error("failed to resolve executable path")]
    UnresolvedExecutable(#[from] std::io::Error),
}

pub(crate) fn build_agent_config() -> Result<LaunchAgent, LaunchAgentError> {
    let executable_path = env::current_exe()?;
    LaunchAgent::builder(DEFAULTS_DOMAIN)
        .arg(executable_path.to_string_lossy())
        .run_at_load(true)
        .stdout_path("/tmp/nanomiddleclick.stdout.log")
        .stderr_path("/tmp/nanomiddleclick.stderr.log")
        .build()
        .map_err(LaunchAgentError::BuildFailed)
}
