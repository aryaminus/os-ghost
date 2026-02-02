import PropTypes from "prop-types";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";
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
  const [perfSnapshot, setPerfSnapshot] = useState(null);
  const [actionLedger, setActionLedger] = useState([]);
  const [recentEvents, setRecentEvents] = useState([]);
  const [ledgerLoading, setLedgerLoading] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [ledgerFilter, setLedgerFilter] = useState("all");
  const [ledgerSource, setLedgerSource] = useState("all");
  const [ledgerRisk, setLedgerRisk] = useState("all");
  const [ledgerQuery, setLedgerQuery] = useState("");
  const [eventBusConfig, setEventBusConfig] = useState({ max_events: 300, dedup_ttl_secs_default: 600 });
  const [eventBusSaving, setEventBusSaving] = useState(false);

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

  const refreshActionLedger = useCallback(async () => {
    setLedgerLoading(true);
    try {
      const entries = await safeInvoke(
        "get_action_ledger",
        {
          limit: 200,
          status: ledgerFilter === "all" ? null : ledgerFilter,
          source: ledgerSource === "all" ? null : ledgerSource,
          riskLevel: ledgerRisk === "all" ? null : ledgerRisk,
          query: ledgerQuery.trim() || null,
        },
        []
      );
      setActionLedger(Array.isArray(entries) ? entries : []);
    } finally {
      setLedgerLoading(false);
    }
  }, [ledgerFilter, ledgerSource, ledgerRisk, ledgerQuery]);

  const refreshRecentEvents = useCallback(async () => {
    setEventsLoading(true);
    try {
      const entries = await safeInvoke("get_recent_events", { limit: 200 }, []);
      setRecentEvents(Array.isArray(entries) ? entries : []);
    } finally {
      setEventsLoading(false);
    }
  }, []);

  const refreshEventBusConfig = useCallback(async () => {
    try {
      const config = await invoke("get_event_bus_config");
      if (config) {
        setEventBusConfig({
          max_events: config.max_events,
          dedup_ttl_secs_default: config.dedup_ttl_secs_default ?? 600,
        });
      }
    } catch (err) {
      console.error("Failed to load event bus config", err);
    }
  }, []);

  const saveEventBusConfig = useCallback(async () => {
    setEventBusSaving(true);
    try {
      await invoke("set_event_bus_config", {
        maxEvents: eventBusConfig.max_events,
        dedupTtlSecsDefault: eventBusConfig.dedup_ttl_secs_default,
      });
    } catch (err) {
      console.error("Failed to save event bus config", err);
    } finally {
      setEventBusSaving(false);
    }
  }, [eventBusConfig]);

  useEffect(() => {
    let mounted = true;
    const loadPerf = async () => {
      const snapshot = await invoke("get_perf_snapshot");
      if (mounted) setPerfSnapshot(snapshot);
    };
    loadPerf();
    const timer = setInterval(loadPerf, 30000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    refreshActionLedger();
    refreshRecentEvents();
    refreshEventBusConfig();
  }, [refreshActionLedger, refreshRecentEvents, refreshEventBusConfig]);

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

      <div className="settings-card">
        <div className="card-row">
          <h3>Action ledger</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={refreshActionLedger}
            disabled={ledgerLoading}
          >
            {ledgerLoading ? "Loading…" : "Refresh"}
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={async () => {
              const data = await invoke("export_action_ledger");
              if (data) {
                await navigator.clipboard.writeText(data);
              }
            }}
          >
            Export
          </button>
        </div>
        <div className="settings-grid">
          <div>
            <label className="card-label" htmlFor="ledger-filter">
              Filter
            </label>
            <select
              id="ledger-filter"
              className="text-input"
              value={ledgerFilter}
              onChange={(event) => setLedgerFilter(event.target.value)}
            >
              <option value="all">All</option>
              <option value="pending">Pending</option>
              <option value="approved">Approved</option>
              <option value="denied">Denied</option>
              <option value="executed">Executed</option>
              <option value="failed">Failed</option>
            </select>
          </div>
          <div>
            <label className="card-label" htmlFor="ledger-source">
              Origin
            </label>
            <select
              id="ledger-source"
              className="text-input"
              value={ledgerSource}
              onChange={(event) => setLedgerSource(event.target.value)}
            >
              <option value="all">All</option>
              <option value="intent">Intent</option>
              <option value="sandbox">Sandbox</option>
              <option value="browser">Browser</option>
              <option value="extensions">Extensions</option>
              <option value="skill">Skills</option>
            </select>
          </div>
          <div>
            <label className="card-label" htmlFor="ledger-risk">
              Risk
            </label>
            <select
              id="ledger-risk"
              className="text-input"
              value={ledgerRisk}
              onChange={(event) => setLedgerRisk(event.target.value)}
            >
              <option value="all">All</option>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
            </select>
          </div>
          <div>
            <label className="card-label" htmlFor="ledger-query">
              Search
            </label>
            <input
              id="ledger-query"
              className="text-input"
              value={ledgerQuery}
              onChange={(event) => setLedgerQuery(event.target.value)}
            />
          </div>
        </div>
        {actionLedger.length === 0 ? (
          <p className="card-note">No action ledger entries.</p>
        ) : (
          <div className="list-grid">
            {actionLedger.map((entry) => (
              <div key={`${entry.action_id}-${entry.timestamp}`} className="list-item">
                <div>
                  <strong>{entry.description || entry.action_type}</strong>
                  {entry.reason && <div className="card-note">Why: {entry.reason}</div>}
                </div>
                <span className="status-pill neutral">
                  {new Date(entry.timestamp * 1000).toLocaleTimeString()}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-card">
        <div className="card-row">
          <h3>Event stream</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={saveEventBusConfig}
            disabled={eventBusSaving}
          >
            {eventBusSaving ? "Saving…" : "Save"}
          </button>
            <button
              type="button"
              className="ghost-button"
              onClick={() => invoke("clear_events")}
            >
              Clear events
            </button>
        </div>
        <div className="settings-grid">
          <div>
            <label className="card-label" htmlFor="event-max">
              Max events
            </label>
            <input
              id="event-max"
              className="text-input"
              type="number"
              min="50"
              max="2000"
              value={eventBusConfig.max_events}
              onChange={(event) =>
                setEventBusConfig((prev) => ({ ...prev, max_events: Number(event.target.value) }))
              }
            />
          </div>
          <div>
            <label className="card-label" htmlFor="event-ttl">
              Dedup TTL (sec)
            </label>
            <input
              id="event-ttl"
              className="text-input"
              type="number"
              min="0"
              max="3600"
              value={eventBusConfig.dedup_ttl_secs_default}
              onChange={(event) =>
                setEventBusConfig((prev) => ({
                  ...prev,
                  dedup_ttl_secs_default: Number(event.target.value),
                }))
              }
            />
          </div>
        </div>
        <div className="card-row">
          <h3>Recent events</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={refreshRecentEvents}
            disabled={eventsLoading}
          >
            {eventsLoading ? "Loading…" : "Refresh"}
          </button>
        </div>
        {recentEvents.length === 0 ? (
          <p className="card-note">No recent events.</p>
        ) : (
          <div className="list-grid">
            {recentEvents.map((entry) => (
              <div key={entry.id} className="list-item">
                <div>
                  <strong>{entry.summary}</strong>
                  {entry.detail && <div className="card-note">{entry.detail}</div>}
                </div>
                <span className="status-pill neutral">
                  {new Date(entry.timestamp * 1000).toLocaleTimeString()}
                </span>
              </div>
            ))}
          </div>
        )}
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

      {perfSnapshot && (
        <div className="settings-card">
          <h3>Performance</h3>
          <div className="card-row">
            <span className="card-label">Uptime</span>
            <span className="card-value">{Math.floor(perfSnapshot.app_uptime_secs / 60)} min</span>
          </div>
          <div className="card-row">
            <span className="card-label">Memory</span>
            <span className="card-value">
              {perfSnapshot.memory_bytes
                ? `${Math.round(perfSnapshot.memory_bytes / (1024 * 1024))} MB`
                : "Unavailable"}
            </span>
          </div>
          <div className="card-row">
            <span className="card-label">CPU</span>
            <span className="card-value">
              {perfSnapshot.cpu_usage !== null && perfSnapshot.cpu_usage !== undefined
                ? `${perfSnapshot.cpu_usage.toFixed(1)}%`
                : "Unavailable"}
            </span>
          </div>
          <div className="card-row">
            <span className="card-label">Load avg</span>
            <span className="card-value">
              {perfSnapshot.load_avg !== null && perfSnapshot.load_avg !== undefined
                ? perfSnapshot.load_avg.toFixed(2)
                : "Unavailable"}
            </span>
          </div>
          <div className="card-row">
            <span className="card-label">Battery</span>
            <span className="card-value">
              {perfSnapshot.battery_percent !== null && perfSnapshot.battery_percent !== undefined
                ? `${perfSnapshot.battery_percent}%`
                : "Unavailable"}
            </span>
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
