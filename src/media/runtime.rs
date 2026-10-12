//! Select the packaged media interpreter without changing local development.

pub fn python() -> tokio::process::Command {
    let executable = std::env::var_os("PAPO_PYTHON")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/bin/python3".into());
    tokio::process::Command::new(executable)
}
