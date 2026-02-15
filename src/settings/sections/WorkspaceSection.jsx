import PropTypes from "prop-types";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";

const WorkspaceSection = ({ settingsState, onSettingsUpdated }) => {
  const [workspaceContext, setWorkspaceContext] = useState(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toolsPath, setToolsPath] = useState("");
  const [agentsPath, setAgentsPath] = useState("");
  const [bootPath, setBootPath] = useState("");
  const [message, setMessage] = useState("");

  const refreshWorkspace = useCallback(async () => {
    setLoading(true);
    try {
      const context = await safeInvoke("get_workspace_context_files", {}, null);
      setWorkspaceContext(context);
      const tools = await safeInvoke("get_tools_md_path", {}, "");
      const agents = await safeInvoke("get_agents_md_path", {}, "");
      const boot = await safeInvoke("get_boot_md_path", {}, "");
      setToolsPath(tools);
      setAgentsPath(agents);
      setBootPath(boot);
    } catch (err) {
      console.error("Failed to load workspace context", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshWorkspace();
  }, [refreshWorkspace]);

  const handleReload = useCallback(async () => {
    setLoading(true);
    try {
      await invoke("reload_workspace_context_cmd");
      await refreshWorkspace();
      setMessage("Workspace context reloaded");
    } catch (err) {
      console.error("Failed to reload workspace context", err);
      setMessage("Failed to reload");
    } finally {
      setLoading(false);
    }
  }, [refreshWorkspace]);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Workspace Context</h2>
        <p>Manage workspace context files (Moltis-inspired).</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <h3>Context Files</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={handleReload}
            disabled={loading}
          >
            {loading ? "Loading…" : "Reload"}
          </button>
        </div>
        <p className="card-note">
          These files are loaded from your data directory and injected into agent prompts.
        </p>
        {message && <p className="card-note">{message}</p>}
      </div>

      <div className="settings-card">
        <h3>TOOLS.md</h3>
        <p className="card-note">
          Tool notes and policies injected into system prompt. Loaded from: <code>{toolsPath}</code>
        </p>
        <div className="card-row">
          <span className="card-label">Status</span>
          <span className="card-value">
            {workspaceContext?.tools_md ? (
              <span className="text-success">Loaded ({workspaceContext.tools_md.length} chars)</span>
            ) : (
              <span className="text-muted">Not found</span>
            )}
          </span>
        </div>
        <p className="card-note">
          Create a TOOLS.md file in your OS Ghost data directory to add custom tool instructions.
        </p>
      </div>

      <div className="settings-card">
        <h3>AGENTS.md</h3>
        <p className="card-note">
          Workspace-level agent instructions. Loaded from: <code>{agentsPath}</code>
        </p>
        <div className="card-row">
          <span className="card-label">Status</span>
          <span className="card-value">
            {workspaceContext?.agents_md ? (
              <span className="text-success">Loaded ({workspaceContext.agents_md.length} chars)</span>
            ) : (
              <span className="text-muted">Not found</span>
            )}
          </span>
        </div>
        <p className="card-note">
          Create an AGENTS.md file in your OS Ghost data directory to customize agent behavior.
        </p>
      </div>

      <div className="settings-card">
        <h3>BOOT.md</h3>
        <p className="card-note">
          Startup tasks executed on GatewayStart. Loaded from: <code>{bootPath}</code>
        </p>
        <div className="card-row">
          <span className="card-label">Status</span>
          <span className="card-value">
            {workspaceContext?.boot_md ? (
              <span className="text-success">Loaded ({workspaceContext.boot_md.length} chars)</span>
            ) : (
              <span className="text-muted">Not found</span>
            )}
          </span>
        </div>
        <p className="card-note">
          Create a BOOT.md file in your OS Ghost data directory to run tasks on startup.
        </p>
      </div>

      <div className="settings-card">
        <h3>Data Directory Locations</h3>
        <div className="settings-grid">
          <div>
            <span className="card-label">macOS</span>
            <code className="code-block">~/Library/Application Support/os-ghost/</code>
          </div>
          <div>
            <span className="card-label">Linux</span>
            <code className="code-block">~/.config/os-ghost/</code>
          </div>
          <div>
            <span className="card-label">Windows</span>
            <code className="code-block">%APPDATA%\os-ghost\</code>
          </div>
        </div>
      </div>

      <div className="settings-card">
        <h3>Example: TOOLS.md</h3>
        <pre className="code-block">
{`# Custom Tools

## Shell Commands
- Only use \`ls -la\` for directory listing
- Never delete files without confirmation

## Browser
- Prefer GET requests over POST for reading content

## File Operations
- Always use relative paths from workspace root
`}
        </pre>
      </div>

      <div className="settings-card">
        <h3>Example: AGENTS.md</h3>
        <pre className="code-block">
{`# Agent Behavior

## Puzzle Agent
- Focus on finding hidden clues
- Use visual similarity matching for URLs

## Narrator
- Keep dialogue short and mysterious
- Reference previous discoveries when relevant
`}
        </pre>
      </div>

      <div className="settings-card">
        <h3>Example: BOOT.md</h3>
        <pre className="code-block">
{`# Startup Tasks

1. Check for new browser tabs
2. Analyze current screen
3. Greet user with context-aware message
`}
        </pre>
      </div>
    </section>
  );
};

WorkspaceSection.propTypes = {
  settingsState: PropTypes.object,
  onSettingsUpdated: PropTypes.func,
};

export default WorkspaceSection;
