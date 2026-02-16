import { useEffect, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";
import ApiKeyInput from "../components/ApiKeyInput.jsx";

const KeysSection = ({ settingsState, onSettingsUpdated }) => {
  const apiKeySource = settingsState.systemStatus?.apiKeySource || "none";
  const [capabilities, setCapabilities] = useState(null);
  const [tokenUsage, setTokenUsage] = useState(null);
  const [configuredProviders, setConfiguredProviders] = useState([]);
  const [anthropicKey, setAnthropicKey] = useState("");
  const [openaiKey, setOpenaiKey] = useState("");
  const [savingKey, setSavingKey] = useState(null);

  useEffect(() => {
    const loadCapabilities = async () => {
      const caps = await safeInvoke("get_model_capabilities", {}, null);
      if (caps) setCapabilities(caps);
    };
    const loadUsage = async () => {
      const usage = await safeInvoke("get_token_usage", {}, null);
      if (usage) setTokenUsage(usage);
    };
    const loadProviders = async () => {
      const providers = await safeInvoke("get_configured_providers", {}, []);
      setConfiguredProviders(Array.isArray(providers) ? providers : []);
    };
    loadCapabilities();
    loadUsage();
    loadProviders();
  }, []);

  const handleResetUsage = async () => {
    await invoke("reset_token_usage");
    const usage = await safeInvoke("get_token_usage", {}, null);
    if (usage) setTokenUsage(usage);
  };

  const saveProviderKey = async (provider) => {
    setSavingKey(provider);
    try {
      const key = provider === "anthropic" ? anthropicKey : openaiKey;
      await invoke("store_provider_api_key", { provider, apiKey: key });
      await invoke("set_api_key", { apiKey: key, source: provider });
      setConfiguredProviders((prev) => [...prev, provider]);
      if (provider === "anthropic") setAnthropicKey("");
      if (provider === "openai") setOpenaiKey("");
      if (onSettingsUpdated) onSettingsUpdated();
    } catch (err) {
      console.error(`Failed to save ${provider} key`, err);
    } finally {
      setSavingKey(null);
    }
  };

  const hasAnthropic = configuredProviders.includes("anthropic");
  const hasOpenai = configuredProviders.includes("openai");

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Keys & Models</h2>
        <p>Configure AI provider API keys.</p>
      </header>

      <div className="settings-card">
        <ApiKeyInput onKeySet={onSettingsUpdated} apiKeySource={apiKeySource} />
      </div>

      <div className="settings-card">
        <h3>Anthropic (Claude)</h3>
        {hasAnthropic ? (
          <div className="card-row">
            <span className="card-value success-text">API key configured</span>
            <button
              type="button"
              className="ghost-button"
              onClick={async () => {
                await invoke("delete_provider_api_key", { provider: "anthropic" });
                setConfiguredProviders((prev) => prev.filter((p) => p !== "anthropic"));
                if (onSettingsUpdated) onSettingsUpdated();
              }}
            >
              Remove
            </button>
          </div>
        ) : (
          <div className="input-group">
            <input
              className="text-input"
              type="password"
              placeholder="sk-ant-api03-..."
              value={anthropicKey}
              onChange={(e) => setAnthropicKey(e.target.value)}
            />
            <button
              type="button"
              onClick={() => saveProviderKey("anthropic")}
              disabled={savingKey === "anthropic" || !anthropicKey.trim()}
            >
              {savingKey === "anthropic" ? "Saving..." : "Save"}
            </button>
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>OpenAI (GPT)</h3>
        {hasOpenai ? (
          <div className="card-row">
            <span className="card-value success-text">API key configured</span>
            <button
              type="button"
              className="ghost-button"
              onClick={async () => {
                await invoke("delete_provider_api_key", { provider: "openai" });
                setConfiguredProviders((prev) => prev.filter((p) => p !== "openai"));
                if (onSettingsUpdated) onSettingsUpdated();
              }}
            >
              Remove
            </button>
          </div>
        ) : (
          <div className="input-group">
            <input
              className="text-input"
              type="password"
              placeholder="sk-..."
              value={openaiKey}
              onChange={(e) => setOpenaiKey(e.target.value)}
            />
            <button
              type="button"
              onClick={() => saveProviderKey("openai")}
              disabled={savingKey === "openai" || !openaiKey.trim()}
            >
              {savingKey === "openai" ? "Saving..." : "Save"}
            </button>
          </div>
        )}
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
