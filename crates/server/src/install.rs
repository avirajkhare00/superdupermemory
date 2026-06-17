use std::path::{Path, PathBuf};

use anyhow::Context;

pub fn run(target_claude: bool, target_cursor: bool, target_codex: bool) -> anyhow::Result<()> {
    let binary = std::env::current_exe()
        .context("cannot determine current binary path")?
        .to_string_lossy()
        .to_string();

    let all = !target_claude && !target_cursor && !target_codex;
    let do_claude = all || target_claude;
    let do_cursor = all || target_cursor;
    let do_codex = all || target_codex;

    if do_claude {
        install_claude_code(&binary)?;
    }
    if do_cursor {
        install_cursor(&binary)?;
    }
    if do_codex {
        print_codex_snippet(&binary);
    }

    Ok(())
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".into())
}

fn install_json_mcp(config_path: &Path, binary: &str) -> anyhow::Result<()> {
    let mut config: serde_json::Value = if config_path.exists() {
        let raw = std::fs::read_to_string(config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", config_path.display()))?
    } else {
        serde_json::json!({})
    };

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    config["mcpServers"]["superdupermemory"] = serde_json::json!({
        "command": binary,
        "args": ["serve"]
    });

    std::fs::write(config_path, serde_json::to_string_pretty(&config)?)
        .with_context(|| format!("writing {}", config_path.display()))?;

    println!("[ok] Claude Code: {}", config_path.display());
    Ok(())
}

fn install_claude_code(binary: &str) -> anyhow::Result<()> {
    let path = PathBuf::from(home()).join(".claude").join("settings.json");
    install_json_mcp(&path, binary).context("installing for Claude Code")
}

fn install_cursor(binary: &str) -> anyhow::Result<()> {
    let path = PathBuf::from(home()).join(".cursor").join("mcp.json");
    // Reuse same JSON shape — Cursor's mcp.json uses identical mcpServers structure.
    let mut config: serde_json::Value = if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?
    } else {
        serde_json::json!({})
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    config["mcpServers"]["superdupermemory"] = serde_json::json!({
        "command": binary,
        "args": ["serve"]
    });

    std::fs::write(&path, serde_json::to_string_pretty(&config)?)
        .with_context(|| format!("writing {}", path.display()))?;

    println!("[ok] Cursor: {}", path.display());
    Ok(())
}

fn print_codex_snippet(binary: &str) {
    println!("[info] Codex CLI — add this to ~/.codex/config.yaml:");
    println!();
    println!("mcp_servers:");
    println!("  - name: superdupermemory");
    println!("    command: {binary}");
    println!("    args:");
    println!("      - serve");
    println!();
}
