import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { useActionManagement } from "../../hooks/useActionManagement";

const DeveloperSection = ({ settingsState }) => {
  const autonomyLevel = settingsState.privacy?.autonomy_level || "observer";
  const apiKeyConfigured = !!settingsState.systemStatus?.apiKeyConfigured;
  const recentTimeline = settingsState.recentTimeline || [];

  const {
    pendingActions,
    actionPreview,
    rollbackStatus,
    tokenUsage,
    modelCapabilities,
    approveAction,
    denyAction,
    undoAction,
    redoAction,
  } = useActionManagement(autonomyLevel, apiKeyConfigured);

  const handleClearPending = async () => {
    await invoke("clear_pending_actions");
  };

  const handleClearHistory = async () => {
    await invoke("clear_action_history");
  };

  const handleResetTokenUsage = async () => {
    await invoke("reset_token_usage");
  };

  const handleClearTimeline = async () => {
    await invoke("clear_timeline");
  };

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Developer</h2>
        <p>Diagnostics, action queues, and rollback tools.</p>
      </header>

      <div className="settings-card">
        <h3>Action queue</h3>
        {pendingActions.length === 0 ? (
          <p className="card-note">No pending actions.</p>
        ) : (
          <div className="list-grid">
            {pendingActions.map((action) => (
              <div key={action.id} className="list-item">
                <div>
                  <strong>{action.action_type}</strong>
                  <div className="card-note">{action.description}</div>
                </div>
                <div className="button-row compact">
                  <button type="button" onClick={() => approveAction(action.id)}>
                    Approve
                  </button>
                  <button type="button" onClick={() => denyAction(action.id)}>
                    Deny
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleClearPending}>
            Clear pending actions
          </button>
        </div>
      </div>

      <div className="settings-card">
        <h3>Action preview</h3>
        {actionPreview ? (
          <div className="preview-card">
            <div className="card-row">
              <span className="card-label">Type</span>
              <span className="card-value">{actionPreview.action?.action_type}</span>
            </div>
            <div className="card-row">
              <span className="card-label">Status</span>
              <span className="card-value">{actionPreview.state}</span>
            </div>
          </div>
        ) : (
          <p className="card-note">No active preview.</p>
        )}
      </div>

      <div className="settings-card">
        <h3>Rollback</h3>
        <div className="card-row">
          <span className="card-label">Undo available</span>
          <span className="card-value">{rollbackStatus.can_undo ? "Yes" : "No"}</span>
        </div>
        <div className="button-row">
          <button
            type="button"
            className="ghost-button"
            onClick={undoAction}
            disabled={!rollbackStatus.can_undo}
          >
            Undo
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={redoAction}
            disabled={!rollbackStatus.can_redo}
          >
            Redo
          </button>
        </div>
      </div>

      <div className="settings-card">
        <h3>Token usage</h3>
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
          <button type="button" className="ghost-button" onClick={handleResetTokenUsage}>
            Reset usage
          </button>
        </div>
      </div>

      <div className="settings-card">
        <h3>Action history</h3>
        <p className="card-note">Clear stored action history for this session.</p>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleClearHistory}>
            Clear history
          </button>
        </div>
      </div>

      <div className="settings-card">
        <h3>Timeline</h3>
        {recentTimeline.length === 0 ? (
          <p className="card-note">No recent timeline entries.</p>
        ) : (
          <div className="list-grid">
            {recentTimeline.map((entry) => (
              <div key={entry.id} className="list-item">
                <div>
                  <strong>{entry.summary}</strong>
                  {entry.reason && <div className="card-note">{entry.reason}</div>}
                </div>
                <span className="status-pill neutral">{entry.status}</span>
              </div>
            ))}
          </div>
        )}
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleClearTimeline}>
            Clear timeline
          </button>
        </div>
      </div>

      {modelCapabilities && (
        <div className="settings-card">
          <h3>Model capabilities</h3>
          <div className="card-row">
            <span className="card-label">Provider</span>
            <span className="card-value">{modelCapabilities.provider}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Vision</span>
            <span className="card-value">{modelCapabilities.has_vision ? "Yes" : "No"}</span>
          </div>
          <div className="card-row">
            <span className="card-label">Tool calling</span>
            <span className="card-value">{modelCapabilities.has_tool_calling ? "Yes" : "No"}</span>
          </div>
        </div>
      )}
    </section>
  );
};

DeveloperSection.propTypes = {
  settingsState: PropTypes.shape({
    privacy: PropTypes.object,
    systemStatus: PropTypes.object,
    recentTimeline: PropTypes.array,
  }).isRequired,
};

export default DeveloperSection;
