import { useEffect, useState } from "react";
import PropTypes from "prop-types";
import { getName, getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

const GeneralSection = ({
  settingsState,
  devModeEnabled,
  onToggleDevMode,
  onOpenSection,
  onSettingsUpdated,
}) => {
  const [appMeta, setAppMeta] = useState({ name: "OS Ghost", version: "" });

  useEffect(() => {
    const loadMeta = async () => {
      try {
        const [name, version] = await Promise.all([getName(), getVersion()]);
        setAppMeta({ name, version });
      } catch {
        setAppMeta((prev) => ({ ...prev, version: "" }));
      }
    };
    loadMeta();
  }, []);

  const privacy = settingsState.privacy;
  const systemStatus = settingsState.systemStatus;
  const systemSettings = settingsState.systemSettings;
  const [shortcutEnabled, setShortcutEnabled] = useState(
    !!settingsState.systemSettings?.global_shortcut_enabled
  );
  const [shortcutError, setShortcutError] = useState("");
  const [shortcutValue, setShortcutValue] = useState(
    settingsState.systemSettings?.global_shortcut || "CmdOrCtrl+Shift+G"
  );

  useEffect(() => {
    setShortcutEnabled(!!settingsState.systemSettings?.global_shortcut_enabled);
    if (settingsState.systemSettings?.global_shortcut) {
      setShortcutValue(settingsState.systemSettings.global_shortcut);
    }
  }, [
    settingsState.systemSettings?.global_shortcut_enabled,
    settingsState.systemSettings?.global_shortcut,
  ]);
  const preferredMode = systemStatus?.preferredMode || "companion";

  const handleModeChange = async (event) => {
    await invoke("set_app_mode", { mode: event.target.value, persistPreference: true });
    onSettingsUpdated?.();
  };

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>General</h2>
        <p>High-level status and system controls.</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Application</span>
          <span className="card-value">{appMeta.name}</span>
        </div>
        <div className="card-row">
          <span className="card-label">Version</span>
          <span className="card-value">{appMeta.version || "—"}</span>
        </div>
      </div>

      <div className="settings-grid">
        <div className="settings-card">
          <div className="card-row">
            <span className="card-label">Consent</span>
            <span className={`status-pill ${privacy?.privacy_notice_acknowledged ? "ok" : "warn"}`}>
              {privacy?.privacy_notice_acknowledged ? "Granted" : "Required"}
            </span>
          </div>
          <p className="card-note">
            Background monitoring is paused until consent is granted.
          </p>
          <button
            type="button"
            className="primary-button"
            onClick={() => onOpenSection("privacy")}
          >
            Review privacy settings
          </button>
        </div>

        <div className="settings-card">
          <div className="card-row">
            <span className="card-label">Autonomy</span>
            <span className="status-pill neutral">
              {privacy?.autonomy_level || "observer"}
            </span>
          </div>
          <p className="card-note">
            Autonomy governs whether the system can execute actions on its own.
          </p>
          <button
            type="button"
            className="ghost-button"
            onClick={() => onOpenSection("autonomy")}
          >
            Manage autonomy
          </button>
        </div>

        <div className="settings-card">
          <div className="card-row">
            <span className="card-label">Extension</span>
            <span className={`status-pill ${systemStatus?.extensionConnected ? "ok" : "warn"}`}>
              {systemStatus?.extensionConnected ? "Connected" : "Not connected"}
            </span>
          </div>
          <p className="card-note">
            Connect the browser extension for real-time context.
          </p>
          <button
            type="button"
            className="ghost-button"
            onClick={() => onOpenSection("extensions")}
          >
            Open extension settings
          </button>
        </div>
      </div>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Developer mode</span>
          <button
            type="button"
            className={`toggle-button ${devModeEnabled ? "active" : ""}`}
            onClick={onToggleDevMode}
            aria-pressed={devModeEnabled}
          >
            {devModeEnabled ? "Enabled" : "Disabled"}
          </button>
        </div>
        <p className="card-note">
          Reveal advanced diagnostics, action queues, and sandbox tools.
        </p>
      </div>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Global shortcut</span>
          <button
            type="button"
            className={`toggle-button ${shortcutEnabled ? "active" : ""}`}
            onClick={async () => {
              setShortcutError("");
              const next = !shortcutEnabled;
              setShortcutEnabled(next);
              try {
                const updated = await invoke("set_global_shortcut_enabled", { enabled: next });
                setShortcutEnabled(!!updated?.global_shortcut_enabled);
                onSettingsUpdated?.();
              } catch (err) {
                console.error("Failed to toggle shortcut", err);
                setShortcutError(typeof err === "string" ? err : "Unable to update shortcut.");
                setShortcutEnabled(!next);
              }
            }}
            aria-pressed={shortcutEnabled}
          >
            {shortcutEnabled ? "Enabled" : "Disabled"}
          </button>
        </div>
        <p className="card-note">Cmd/Ctrl + Shift + G toggles the Ghost window.</p>
        {shortcutError && <span className="status-pill error">{shortcutError}</span>}
        <div className="input-row">
          <input
            className="text-input"
            value={shortcutValue}
            onChange={(event) => setShortcutValue(event.target.value)}
            placeholder="CmdOrCtrl+Shift+G"
          />
          <button
            type="button"
            className="ghost-button"
            onClick={async () => {
              setShortcutError("");
              try {
                const updated = await invoke("set_global_shortcut", { shortcut: shortcutValue });
                setShortcutValue(updated.global_shortcut);
                onSettingsUpdated?.();
              } catch (err) {
                console.error("Failed to set shortcut", err);
                setShortcutError(typeof err === "string" ? err : "Unable to update shortcut.");
              }
            }}
          >
            Apply
          </button>
        </div>
        <p className="card-note">
          If this hotkey conflicts with another app, choose a different combination.
        </p>
      </div>

      <div className="settings-card">
        <h3>Mode preference</h3>
        <p className="card-note">
          Set the default mode when no active puzzle is running.
        </p>
        <div className="input-row">
          <select className="select-control" value={preferredMode} onChange={handleModeChange}>
            <option value="companion">Companion</option>
            <option value="game">Game</option>
          </select>
        </div>
      </div>
    </section>
  );
};

GeneralSection.propTypes = {
  settingsState: PropTypes.shape({
    privacy: PropTypes.object,
    systemStatus: PropTypes.object,
    systemSettings: PropTypes.object,
  }).isRequired,
  devModeEnabled: PropTypes.bool.isRequired,
  onToggleDevMode: PropTypes.func.isRequired,
  onOpenSection: PropTypes.func.isRequired,
  onSettingsUpdated: PropTypes.func,
};

export default GeneralSection;
