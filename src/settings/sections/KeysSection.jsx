import { useEffect, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";
import ApiKeyInput from "../components/ApiKeyInput.jsx";

const KeysSection = ({ settingsState, onSettingsUpdated }) => {
  const apiKeySource = settingsState.systemStatus?.apiKeySource || "none";
  const [capabilities, setCapabilities] = useState(null);
  const [tokenUsage, setTokenUsage] = useState(null);

  useEffect(() => {
    const loadCapabilities = async () => {
      const caps = await safeInvoke("get_model_capabilities", {}, null);
      if (caps) setCapabilities(caps);
    };
    const loadUsage = async () => {
      const usage = await safeInvoke("get_token_usage", {}, null);
      if (usage) setTokenUsage(usage);
    };
    loadCapabilities();
    loadUsage();
  }, []);

  const handleResetUsage = async () => {
    await invoke("reset_token_usage");
    const usage = await safeInvoke("get_token_usage", {}, null);
    if (usage) setTokenUsage(usage);
  };
  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Keys & Models</h2>
        <p>Configure Gemini API access and local models.</p>
      </header>

      <div className="settings-card">
        <ApiKeyInput onKeySet={onSettingsUpdated} apiKeySource={apiKeySource} />
      </div>

      {capabilities && (
        <div className="settings-card">
          <h3>Model capabilities</h3>
          <div className="card-row">
            <span className="card-label">Provider</span>
            <span className="card-value">{capabilities.provider}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Vision</span>
            <span className="card-value">{capabilities.has_vision ? "Yes" : "No"}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Tool calling</span>
            <span className="card-value">{capabilities.has_tool_calling ? "Yes" : "No"}</span>
          </div>
          {capabilities.warnings?.length > 0 && (
            <div className="notice-box">
              {capabilities.warnings.map((warning) => (
                <div key={warning}>{warning}</div>
              ))}
            </div>
          )}
        </div>
      )}

      {tokenUsage && (
        <div className="settings-card">
          <h3>Usage</h3>
          <div className="card-row">
            <span className="card-label">Gemini calls</span>
            <span className="card-value">{tokenUsage.gemini_calls}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Ollama calls</span>
            <span className="card-value">{tokenUsage.ollama_calls}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Estimated cost</span>
            <span className="card-value">${tokenUsage.estimated_cost_usd.toFixed(3)}</span>
          </div>
          <div className="button-row">
            <button type="button" className="ghost-button" onClick={handleResetUsage}>
              Reset usage
            </button>
          </div>
        </div>
      )}
    </section>
  );
};

KeysSection.propTypes = {
  settingsState: PropTypes.shape({
    systemStatus: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default KeysSection;
