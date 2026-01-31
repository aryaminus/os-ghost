import { useCallback, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

const WEB_STORE_URL =
  "https://chromewebstore.google.com/detail/os-ghost-bridge/iakaaklohlcdhoalipmmljopmjnhbcdn";

const ExtensionsSection = ({ settingsState, onSettingsUpdated }) => {
  const [launching, setLaunching] = useState(false);
  const [error, setError] = useState("");

  const status = settingsState.systemStatus;

  const handleRefresh = useCallback(async () => {
    setError("");
    await onSettingsUpdated();
  }, [onSettingsUpdated]);

  const handleLaunchChrome = useCallback(async () => {
    setLaunching(true);
    setError("");
    try {
      await invoke("launch_chrome", { url: null });
    } catch (err) {
      console.error("Failed to launch Chrome", err);
      setError("Could not launch Chrome. Please open it manually.");
    } finally {
      setLaunching(false);
    }
  }, []);

  const handleInstallExtension = useCallback(async () => {
    setError("");
    try {
      await invoke("launch_chrome", { url: WEB_STORE_URL });
    } catch (err) {
      console.error("Failed to open Web Store", err);
      try {
        await openUrl(WEB_STORE_URL);
      } catch {
        setError("Could not open the Web Store page.");
      }
    }
  }, []);

  const handleGetChrome = useCallback(async () => {
    try {
      await openUrl("https://www.google.com/chrome/");
    } catch (err) {
      console.error("Failed to open Chrome download", err);
      setError("Could not open Chrome download page.");
    }
  }, []);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Extensions</h2>
        <p>Connect the browser extension for real-time awareness.</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Browser</span>
          <span className={`status-pill ${status?.chromeInstalled ? "ok" : "warn"}`}>
            {status?.chromeInstalled ? "Installed" : "Not detected"}
          </span>
        </div>
        <div className="card-row">
          <span className="card-label">Extension</span>
          <span className={`status-pill ${status?.extensionConnected ? "ok" : "warn"}`}>
            {status?.extensionConnected ? "Connected" : "Not connected"}
          </span>
        </div>
        <div className="button-row">
          {!status?.chromeInstalled && (
            <button type="button" className="primary-button" onClick={handleGetChrome}>
              Get Chrome
            </button>
          )}
          {status?.chromeInstalled && (
            <button
              type="button"
              className="ghost-button"
              onClick={handleLaunchChrome}
              disabled={launching}
            >
              {launching ? "Launching…" : "Launch Chrome"}
            </button>
          )}
          <button type="button" className="primary-button" onClick={handleInstallExtension}>
            Install extension
          </button>
          <button type="button" className="ghost-button" onClick={handleRefresh}>
            Refresh status
          </button>
        </div>
        {error && <span className="status-pill error">{error}</span>}
      </div>

      {!status?.extensionConnected && (
        <div className="settings-card subtle">
          <p>
            Without the extension, OS Ghost runs in screenshot-only mode and has
            reduced context.
          </p>
        </div>
      )}
    </section>
  );
};

ExtensionsSection.propTypes = {
  settingsState: PropTypes.shape({
    systemStatus: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default ExtensionsSection;
