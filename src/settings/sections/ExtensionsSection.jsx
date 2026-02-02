import { useCallback, useEffect, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

const WEB_STORE_URL =
  "https://chromewebstore.google.com/detail/os-ghost-bridge/iakaaklohlcdhoalipmmljopmjnhbcdn";

const ExtensionsSection = ({ settingsState, onSettingsUpdated }) => {
  const [launching, setLaunching] = useState(false);
  const [error, setError] = useState("");
  const [pairingCode, setPairingCode] = useState("");
  const [pairingExpires, setPairingExpires] = useState(null);
  const [protocolMessage, setProtocolMessage] = useState("");
  const [resetMessage, setResetMessage] = useState("");
  const [buildMessage, setBuildMessage] = useState("");
  const [extensionTools, setExtensionTools] = useState([]);

  const status = settingsState.systemStatus;
  const pairing = settingsState.pairingStatus;

  useEffect(() => {
    setPairingCode(pairing?.pending_code || "");
    setPairingExpires(pairing?.pending_expires_at || null);
  }, [pairing?.pending_code, pairing?.pending_expires_at]);

  useEffect(() => {
    const loadTools = async () => {
      try {
        const tools = await invoke("list_extension_tools");
        setExtensionTools(Array.isArray(tools) ? tools : []);
      } catch (err) {
        console.error("Failed to load extension tools", err);
      }
    };
    loadTools();
  }, []);

  const handleRefresh = useCallback(async () => {
    setError("");
    setProtocolMessage("");
    setResetMessage("");
    setBuildMessage("");
    await onSettingsUpdated();
    try {
      const tools = await invoke("list_extension_tools");
      setExtensionTools(Array.isArray(tools) ? tools : []);
    } catch (err) {
      console.error("Failed to load extension tools", err);
    }
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

  const handleGeneratePairing = useCallback(async () => {
    setError("");
    try {
      const next = await invoke("create_pairing_code");
      setPairingCode(next?.pending_code || "");
      setPairingExpires(next?.pending_expires_at || null);
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to create pairing code", err);
      setError("Could not generate a pairing code.");
    }
  }, [onSettingsUpdated]);

  const handleReconnect = useCallback(async () => {
    setError("");
    try {
      await invoke("request_extension_ping");
      await onSettingsUpdated();
    } catch (err) {
      console.error("Failed to ping extension", err);
      setError("Unable to reach extension. Try launching Chrome.");
    }
  }, [onSettingsUpdated]);

  const handleCheckProtocol = useCallback(() => {
    const expected = "1";
    if (!status?.extensionProtocolVersion && !status?.lastExtensionHello) {
      setProtocolMessage("No protocol handshake detected yet.");
      return;
    }
    if (status.extensionProtocolVersion === "legacy") {
      setProtocolMessage("Legacy extension detected. Update to latest build for handshake support.");
      return;
    }
    if (!status?.extensionProtocolVersion && status?.lastExtensionHello) {
      setProtocolMessage("Handshake received but no protocol details. Update extension.");
      return;
    }
    if (status.extensionProtocolVersion === expected) {
      setProtocolMessage("Protocol OK.");
    } else {
      setProtocolMessage(`Protocol mismatch: expected ${expected}, got ${status.extensionProtocolVersion}.`);
    }
  }, [status?.extensionProtocolVersion]);

  const handleResetBridge = useCallback(async () => {
    setError("");
    setResetMessage("");
    setBuildMessage("");
    try {
      await invoke("reset_bridge_registration");
      setResetMessage("Bridge registration refreshed. Reload the extension.");
      await onSettingsUpdated();
    } catch (err) {
      console.error("Failed to reset bridge registration", err);
      setError("Reset failed. Ensure native_bridge is built and try again.");
    }
  }, [onSettingsUpdated]);

  const handleRebuildBridge = useCallback(async () => {
    setError("");
    setBuildMessage("");
    try {
      await invoke("rebuild_native_bridge");
      setBuildMessage("native_bridge rebuilt. Now reset registration and reload the extension.");
    } catch (err) {
      console.error("Failed to rebuild native bridge", err);
      setError("Rebuild failed. Ensure cargo is installed and try again.");
    }
  }, []);

  const handleClearPairing = useCallback(async () => {
    setError("");
    try {
      const next = await invoke("clear_pairing_code");
      setPairingCode(next?.pending_code || "");
      setPairingExpires(next?.pending_expires_at || null);
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to clear pairing code", err);
      setError("Could not clear pairing code.");
    }
  }, [onSettingsUpdated]);

  const isDev = import.meta.env.DEV;

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
        <div className="card-row">
          <span className="card-label">Operational</span>
          <span className={`status-pill ${status?.extensionOperational ? "ok" : "warn"}`}>
            {status?.extensionOperational ? "Healthy" : "Stale"}
          </span>
        </div>
        {status?.extensionVersion && (
          <div className="card-row">
            <span className="card-label">Extension version</span>
            <span className="card-value">{status.extensionVersion}</span>
          </div>
        )}
        {status?.extensionProtocolVersion && (
          <div className="card-row">
            <span className="card-label">Protocol</span>
            <span className="card-value">{status.extensionProtocolVersion}</span>
          </div>
        )}
        {status?.lastExtensionHeartbeat && (
          <div className="card-row">
            <span className="card-label">Last heartbeat</span>
            <span className="card-value">
              {new Date(status.lastExtensionHeartbeat * 1000).toLocaleTimeString()}
            </span>
          </div>
        )}
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
          <button
            type="button"
            className="ghost-button"
            onClick={() => invoke("request_extension_ping")}
          >
            Ping extension
          </button>
          <button type="button" className="ghost-button" onClick={handleReconnect}>
            Reconnect
          </button>
          <button type="button" className="ghost-button" onClick={handleCheckProtocol}>
            Check protocol
          </button>
          <button type="button" className="ghost-button" onClick={handleResetBridge}>
            Reset bridge registration
          </button>
          {isDev && (
            <button type="button" className="ghost-button" onClick={handleRebuildBridge}>
              Rebuild native bridge
            </button>
          )}
        </div>
        {error && <span className="status-pill error">{error}</span>}
        {protocolMessage && <span className="status-pill neutral">{protocolMessage}</span>}
        {resetMessage && <span className="status-pill neutral">{resetMessage}</span>}
        {buildMessage && <span className="status-pill neutral">{buildMessage}</span>}
      </div>

      <div className="settings-card">
        <h3>Extension tools</h3>
        {extensionTools.length === 0 ? (
          <p className="card-note">No tools registered yet.</p>
        ) : (
          <div className="list-grid">
            {extensionTools.map((toolset) => (
              <div key={toolset.extension_id} className="list-item">
                <div>
                  <strong>{toolset.extension_id}</strong>
                  {toolset.tools?.map((tool) => (
                    <div key={`${toolset.extension_id}-${tool.name}`} className="card-note">
                      {tool.name} · {tool.description}
                      {tool.args_schema && (
                        <div className="card-note">Schema: {JSON.stringify(tool.args_schema)}</div>
                      )}
                      {tool.risk_level && (
                        <div className="card-note">Risk: {tool.risk_level}</div>
                      )}
                      {tool.requires_approval && <div className="card-note">Approval required</div>}
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>Pairing</h3>
        <p className="card-note">Pairing codes are required for future remote channels.</p>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleGeneratePairing}>
            Generate code
          </button>
          <button type="button" className="ghost-button" onClick={handleClearPairing}>
            Clear code
          </button>
        </div>
        {pairingCode && (
          <div className="card-row">
            <span className="card-label">Pairing code</span>
            <span className="card-value">{pairingCode}</span>
          </div>
        )}
        {pairingExpires && (
          <div className="card-row">
            <span className="card-label">Expires</span>
            <span className="card-value">{new Date(pairingExpires * 1000).toLocaleTimeString()}</span>
          </div>
        )}
        {pairing?.trusted_sources?.length > 0 ? (
          <div className="list-grid">
            {pairing.trusted_sources.map((source) => (
              <div key={`${source.source_type}-${source.id}`} className="list-item">
                <div>
                  <strong>{source.label}</strong>
                  <div className="card-note">{source.source_type}</div>
                </div>
                <span className="status-pill neutral">Trusted</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="card-note">No trusted sources registered.</p>
        )}
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
    pairingStatus: PropTypes.object,
    extensionTools: PropTypes.array,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default ExtensionsSection;
