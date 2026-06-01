use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::repo;

pub const NO_ENV: &[(&str, &str)] = &[];

pub fn script(root: &Path, name: &str, env: &[(&str, &str)]) -> Result<()> {
    let path = repo::script_path(root, name);
    if !path.exists() {
        bail!("flow script not found: {}", path.display());
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&path).current_dir(root);
    for (key, value) in env {
        cmd.env(key, value);
    }

    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {}", path.display()))?;
    if !status.success() {
        bail!("{name} exited with {status}");
    }
    Ok(())
}

pub fn script_exists(root: &Path, name: &str) -> bool {
    repo::script_path(root, name).exists()
}

pub fn command(root: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("failed to spawn {program}"))?;
    if !status.success() {
        bail!("{program} {} exited with {status}", args.join(" "));
    }
    Ok(())
}

pub fn piped(root: &Path, program: &str, args: &[&str], input: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn {program}"))?;

    child
        .stdin
        .as_mut()
        .context("child stdin unavailable")?
        .write_all(input.as_bytes())
        .context("failed to write to child stdin")?;

    let status = child.wait().with_context(|| format!("{program} failed"))?;

    if !status.success() {
        bail!("{program} exited with {status}");
    }

    Ok(())
}

pub fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to spawn {program}"))?;

    if !output.status.success() {
        bail!("{program} exited with {}", output.status);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn capture_all(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to spawn {program}"))?;

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        bail!("{program} exited with {}:\n{combined}", output.status);
    }

    Ok(combined)
}

pub fn compose(root: &Path, extra: &[&str]) -> Result<()> {
    let mut args = vec!["compose", "--profile", "local", "--profile", "hermes"];
    args.extend_from_slice(extra);

    command(root, "docker", &args)
}

pub fn docker_login(username: &str, token: &str) -> Result<()> {
    let mut child = Command::new("docker")
        .args(["login", "-u", username, "--password-stdin"])
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to spawn docker login")?;

    child
        .stdin
        .as_mut()
        .context("docker login stdin unavailable")?
        .write_all(token.as_bytes())
        .context("failed to write docker token")?;

    let status = child.wait().context("docker login failed")?;

    if !status.success() {
        bail!("docker login exited with {status}");
    }

    Ok(())
}

pub fn has(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
