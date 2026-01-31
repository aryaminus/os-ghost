import { useCallback, useEffect, useMemo, useState } from "react";
import PropTypes from "prop-types";
import { safeInvoke } from "../../utils/data";

const TRUST_LEVELS = [
  { value: "untrusted", label: "Untrusted" },
  { value: "read_only", label: "Read-only" },
  { value: "limited", label: "Limited" },
  { value: "elevated", label: "Elevated" },
  { value: "full", label: "Full" },
];

const SHELL_CATEGORIES = [
  { value: "read_info", label: "Read info" },
  { value: "search", label: "Search" },
  { value: "package_info", label: "Package info" },
  { value: "git_read", label: "Git read" },
  { value: "git_write", label: "Git write" },
  { value: "file_manipulation", label: "File manipulation" },
  { value: "file_deletion", label: "File deletion" },
  { value: "network", label: "Network" },
  { value: "process_management", label: "Process management" },
  { value: "system_admin", label: "System admin" },
  { value: "arbitrary", label: "Arbitrary" },
];

const SandboxSection = ({ settingsState, onSettingsUpdated }) => {
  const [sandbox, setSandbox] = useState(null);
  const [readPath, setReadPath] = useState("");
  const [writePath, setWritePath] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (settingsState.sandboxSettings) {
      setSandbox(settingsState.sandboxSettings);
    }
  }, [settingsState.sandboxSettings]);

  const allowedShell = useMemo(() => {
    if (!sandbox?.allowed_shell_categories) return new Set();
    return new Set(sandbox.allowed_shell_categories);
  }, [sandbox?.allowed_shell_categories]);

  const updateSandbox = useCallback(async (command, args) => {
    setError("");
    const updated = await safeInvoke(command, args, null);
    if (updated) {
      setSandbox(updated);
    } else {
      setError("Unable to update sandbox settings.");
    }
  }, []);

  const handleTrustChange = useCallback(
    (event) => updateSandbox("set_sandbox_trust_level", { level: event.target.value }),
    [updateSandbox]
  );

  const handleAddRead = useCallback(() => {
    if (!readPath.trim()) return;
    updateSandbox("add_sandbox_read_path", { path: readPath.trim() });
    setReadPath("");
  }, [readPath, updateSandbox]);

  const handleAddWrite = useCallback(() => {
    if (!writePath.trim()) return;
    updateSandbox("add_sandbox_write_path", { path: writePath.trim() });
    setWritePath("");
  }, [writePath, updateSandbox]);

  const handleRemoveRead = useCallback(
    (path) => updateSandbox("remove_sandbox_read_path", { path }),
    [updateSandbox]
  );

  const handleRemoveWrite = useCallback(
    (path) => updateSandbox("remove_sandbox_write_path", { path }),
    [updateSandbox]
  );

  const handleToggleShell = useCallback(
    (category) => {
      const enabled = allowedShell.has(category);
      updateSandbox(enabled ? "disable_shell_category" : "enable_shell_category", { category });
    },
    [allowedShell, updateSandbox]
  );

  if (!sandbox) {
    return (
      <section className="settings-section">
        <header className="section-header">
          <h2>Sandbox</h2>
          <p>Loading sandbox settings…</p>
        </header>
      </section>
    );
  }

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Sandbox</h2>
        <p>Control file and shell access boundaries.</p>
      </header>

      <div className="settings-card">
        <h3>Trust level</h3>
        <select className="select-control" value={sandbox.trust_level} onChange={handleTrustChange}>
          {TRUST_LEVELS.map((level) => (
            <option key={level.value} value={level.value}>
              {level.label}
            </option>
          ))}
        </select>
        <p className="card-note">
          Trust score: {sandbox.trust_score} · Safe ops: {sandbox.safe_operations_count}
        </p>
      </div>

      <div className="settings-card">
        <h3>Write confirmations</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={!!sandbox.confirm_all_writes}
            onChange={(event) =>
              updateSandbox("set_confirm_all_writes", { enabled: event.target.checked })
            }
          />
          <span>Require confirmation for every write.</span>
        </label>
      </div>

      <div className="settings-card">
        <h3>Max read size (bytes)</h3>
        <div className="input-row">
          <input
            className="text-input"
            type="number"
            min="1024"
            max="52428800"
            value={sandbox.max_read_size}
            onChange={(event) =>
              updateSandbox("set_max_read_size", { maxReadSize: Number(event.target.value) })
            }
          />
        </div>
        <p className="card-note">Limit large file reads for safety and performance.</p>
      </div>

      <div className="settings-card">
        <h3>Read allowlist</h3>
        <div className="input-row">
          <input
            className="text-input"
            value={readPath}
            onChange={(event) => setReadPath(event.target.value)}
            placeholder="/path/to/allow"
          />
          <button type="button" className="ghost-button" onClick={handleAddRead}>
            Add
          </button>
        </div>
        <div className="list-grid">
          {sandbox.read_allowlist?.map((path) => (
            <div key={path} className="list-item">
              <span>{path}</span>
              <button type="button" onClick={() => handleRemoveRead(path)}>
                Remove
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className="settings-card">
        <h3>Write allowlist</h3>
        <div className="input-row">
          <input
            className="text-input"
            value={writePath}
            onChange={(event) => setWritePath(event.target.value)}
            placeholder="/path/to/allow"
          />
          <button type="button" className="ghost-button" onClick={handleAddWrite}>
            Add
          </button>
        </div>
        <div className="list-grid">
          {sandbox.write_allowlist?.map((path) => (
            <div key={path} className="list-item">
              <span>{path}</span>
              <button type="button" onClick={() => handleRemoveWrite(path)}>
                Remove
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className="settings-card">
        <h3>Shell categories</h3>
        <div className="chip-grid">
          {SHELL_CATEGORIES.map((category) => (
            <button
              key={category.value}
              type="button"
              className={`chip ${allowedShell.has(category.value) ? "active" : ""}`}
              onClick={() => handleToggleShell(category.value)}
            >
              {category.label}
            </button>
          ))}
        </div>
      </div>

      {error && <div className="settings-card error">{error}</div>}
    </section>
  );
};

SandboxSection.propTypes = {
  settingsState: PropTypes.shape({
    sandboxSettings: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default SandboxSection;
