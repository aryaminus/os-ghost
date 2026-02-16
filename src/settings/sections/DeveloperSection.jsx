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

  // Hook system state (Moltis-inspired)
  const [hooks, setHooks] = useState([]);
  const [hooksLoading, setHooksLoading] = useState(false);
  const [hookState, setHookState] = useState({});
  const [hooksReloading, setHooksReloading] = useState(false);

  // Config validation state (Moltis-inspired)
  const [configValidation, setConfigValidation] = useState(null);
  const [validatingConfig, setValidatingConfig] = useState(false);

  // Sanitization state (Moltis-inspired)
  const [sanitizerMaxSize, setSanitizerMaxSize] = useState(51200);
  const [sanitizerLoading, setSanitizerLoading] = useState(false);

  // Security state (IronClaw-inspired)
  const [allowlistEnabled, setAllowlistEnabled] = useState(false);
  const [allowedDomains, setAllowedDomains] = useState([]);
  const [securityLoading, setSecurityLoading] = useState(false);
  const [newDomain, setNewDomain] = useState("");
  const [leakTestResult, setLeakTestResult] = useState(null);

  // Tunnel state (ZeroClaw-inspired)
  const [tunnelConfig, setTunnelConfig] = useState({ provider: "none", cloudflareToken: "", tailscaleHostname: "", ngrokToken: "" });
  const [tunnelUrl, setTunnelUrl] = useState(null);
  const [tunnelRunning, setTunnelRunning] = useState(false);

  // Observability state (ZeroClaw-inspired)
  const [metrics, setMetrics] = useState(null);
  const [traces, setTraces] = useState([]);
  const [observabilityLoading, setObservabilityLoading] = useState(false);

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

  // Hook system handlers (Moltis-inspired)
  const refreshHooks = useCallback(async () => {
    setHooksLoading(true);
    try {
      const hookList = await invoke("get_hooks");
      setHooks(Array.isArray(hookList) ? hookList : []);
      const state = await invoke("get_hook_state");
      setHookState(state || {});
    } catch (err) {
      console.error("Failed to load hooks", err);
    } finally {
      setHooksLoading(false);
    }
  }, []);

  const reloadHooks = useCallback(async () => {
    setHooksReloading(true);
    try {
      await invoke("reload_hooks_cmd");
      await refreshHooks();
    } catch (err) {
      console.error("Failed to reload hooks", err);
    } finally {
      setHooksReloading(false);
    }
  }, [refreshHooks]);

  const toggleHook = useCallback(async (hookName, enabled) => {
    try {
      if (enabled) {
        await invoke("enable_hook_cmd", { name: hookName });
      } else {
        await invoke("disable_hook_cmd", { name: hookName });
      }
      await refreshHooks();
    } catch (err) {
      console.error("Failed to toggle hook", err);
    }
  }, [refreshHooks]);

  // Config validation handler (Moltis-inspired)
  const validateConfig = useCallback(async () => {
    setValidatingConfig(true);
    try {
      const result = await invoke("validate_toml_settings");
      setConfigValidation(result);
    } catch (err) {
      console.error("Failed to validate config", err);
      setConfigValidation({ valid: false, errors: [err.toString()], warnings: [], suggestions: [] });
    } finally {
      setValidatingConfig(false);
    }
  }, []);

  // Sanitization handlers (Moltis-inspired)
  const refreshSanitizerSettings = useCallback(async () => {
    setSanitizerLoading(true);
    try {
      const maxSize = await invoke("get_sanitizer_max_size");
      setSanitizerMaxSize(maxSize || 51200);
    } catch (err) {
      console.error("Failed to load sanitizer settings", err);
    } finally {
      setSanitizerLoading(false);
    }
  }, []);

  const saveSanitizerMaxSize = useCallback(async () => {
    setSanitizerLoading(true);
    try {
      await invoke("sanitize_output_with_limit", { content: "", maxBytes: sanitizerMaxSize });
    } catch (err) {
      console.error("Failed to save sanitizer settings", err);
    } finally {
      setSanitizerLoading(false);
    }
  }, [sanitizerMaxSize]);

  // Security handlers (IronClaw-inspired)
  const refreshSecuritySettings = useCallback(async () => {
    setSecurityLoading(true);
    try {
      const enabled = await invoke("get_allowlist_status");
      setAllowlistEnabled(enabled);
      const domains = await invoke("get_allowed_domains_list");
      setAllowedDomains(Array.isArray(domains) ? domains : []);
    } catch (err) {
      console.error("Failed to load security settings", err);
    } finally {
      setSecurityLoading(false);
    }
  }, []);

  const toggleAllowlist = useCallback(async (enabled) => {
    setSecurityLoading(true);
    try {
      await invoke("set_allowlist_enabled", { enabled });
      setAllowlistEnabled(enabled);
    } catch (err) {
      console.error("Failed to toggle allowlist", err);
    } finally {
      setSecurityLoading(false);
    }
  }, []);

  const addAllowedDomain = useCallback(async () => {
    if (!newDomain.trim()) return;
    setSecurityLoading(true);
    try {
      await invoke("add_allowed_domain", { domain: newDomain.trim() });
      setNewDomain("");
      await refreshSecuritySettings();
    } catch (err) {
      console.error("Failed to add domain", err);
    } finally {
      setSecurityLoading(false);
    }
  }, [newDomain, refreshSecuritySettings]);

  const blockDomain = useCallback(async (domain) => {
    setSecurityLoading(true);
    try {
      await invoke("add_blocked_domain", { domain });
      await refreshSecuritySettings();
    } catch (err) {
      console.error("Failed to block domain", err);
    } finally {
      setSecurityLoading(false);
    }
  }, [refreshSecuritySettings]);

  const testLeakDetection = useCallback(async () => {
    setSecurityLoading(true);
    try {
      const testContent = "API Key: sk-1234567890abcdefghijklmnopqrstuvwxyz";
      const result = await invoke("detect_leaks", { content: testContent });
      setLeakTestResult(result);
    } catch (err) {
      console.error("Failed to test leak detection", err);
      setLeakTestResult({ blocked: false, matches: [], error: err.toString() });
    } finally {
      setSecurityLoading(false);
    }
  }, []);

  // Tunnel handlers (ZeroClaw-inspired)
  const refreshTunnel = useCallback(async () => {
    setSecurityLoading(true);
    try {
      const running = await invoke("is_tunnel_running");
      setTunnelRunning(running);
      const url = await invoke("get_tunnel_url");
      setTunnelUrl(url);
    } catch (err) {
      console.error("Failed to refresh tunnel", err);
    } finally {
      setSecurityLoading(false);
    }
  }, []);

  const startTunnel = useCallback(async () => {
    setSecurityLoading(true);
    try {
      await invoke("configure_tunnel", { config: tunnelConfig });
      await invoke("start_tunnel", { localHost: "127.0.0.1", localPort: 7842 });
      await refreshTunnel();
    } catch (err) {
      console.error("Failed to start tunnel", err);
    } finally {
      setSecurityLoading(false);
    }
  }, [tunnelConfig, refreshTunnel]);

  const stopTunnel = useCallback(async () => {
    setSecurityLoading(true);
    try {
      await invoke("stop_tunnel");
      await refreshTunnel();
    } catch (err) {
      console.error("Failed to stop tunnel", err);
    } finally {
      setSecurityLoading(false);
    }
  }, [refreshTunnel]);

  // Observability handlers (ZeroClaw-inspired)
  const refreshMetrics = useCallback(async () => {
    setObservabilityLoading(true);
    try {
      const m = await invoke("get_current_metrics");
      setMetrics(m);
      const t = await invoke("get_traces", { limit: 10 });
      setTraces(Array.isArray(t) ? t : []);
    } catch (err) {
      console.error("Failed to load metrics", err);
    } finally {
      setObservabilityLoading(false);
    }
  }, []);

  const resetMetrics = useCallback(async () => {
    setObservabilityLoading(true);
    try {
      await invoke("metrics_reset");
      await invoke("traces_clear");
      await refreshMetrics();
    } catch (err) {
      console.error("Failed to reset metrics", err);
    } finally {
      setObservabilityLoading(false);
    }
  }, [refreshMetrics]);

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
    refreshHooks();
    refreshSanitizerSettings();
    refreshSecuritySettings();
    refreshTunnel();
    refreshMetrics();
  }, [refreshActionLedger, refreshRecentEvents, refreshEventBusConfig, refreshHooks, refreshSanitizerSettings, refreshSecuritySettings, refreshTunnel, refreshMetrics]);

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

      {/* Hook System Section (Moltis-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Hooks</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={reloadHooks}
            disabled={hooksReloading}
          >
            {hooksReloading ? "Reloading…" : "Reload"}
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={refreshHooks}
            disabled={hooksLoading}
          >
            {hooksLoading ? "Loading…" : "Refresh"}
          </button>
        </div>
        <p className="card-note">
          Lifecycle hooks for observing, modifying, or blocking agent behavior.
        </p>
        {hooks.length === 0 ? (
          <p className="card-note">No hooks discovered. Create ~/.os-ghost/hooks/&lt;name&gt;/HOOK.md</p>
        ) : (
          <div className="list-grid">
            {hooks.map((hook) => (
              <div key={hook.name} className="list-item">
                <div>
                  <strong>{hook.name}</strong>
                  <div className="card-note">
                    Events: {hook.events?.join(", ") || "none"} | Timeout: {hook.timeout_secs}s
                  </div>
                  {hookState[hook.name] && (
                    <span className="status-pill negative">Disabled (circuit breaker)</span>
                  )}
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={hook.enabled !== false}
                    onChange={(e) => toggleHook(hook.name, e.target.checked)}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Config Validation Section (Moltis-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Config Validation</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={validateConfig}
            disabled={validatingConfig}
          >
            {validatingConfig ? "Validating…" : "Validate"}
          </button>
        </div>
        <p className="card-note">
          Validate TOML configuration for errors, warnings, and suggestions.
        </p>
        {configValidation && (
          <div className="preview-card">
            {configValidation.errors?.length > 0 && (
              <div>
                <strong className="text-error">Errors:</strong>
                <ul className="card-list">
                  {configValidation.errors.map((err, i) => (
                    <li key={i} className="text-error">{err}</li>
                  ))}
                </ul>
              </div>
            )}
            {configValidation.warnings?.length > 0 && (
              <div>
                <strong className="text-warning">Warnings:</strong>
                <ul className="card-list">
                  {configValidation.warnings.map((warn, i) => (
                    <li key={i} className="text-warning">{warn}</li>
                  ))}
                </ul>
              </div>
            )}
            {configValidation.suggestions?.length > 0 && (
              <div>
                <strong>Suggestions:</strong>
                <ul className="card-list">
                  {configValidation.suggestions.map((sug, i) => (
                    <li key={i}>{sug}</li>
                  ))}
                </ul>
              </div>
            )}
            {configValidation.valid && !configValidation.errors?.length && (
              <p className="card-note text-success">Configuration is valid!</p>
            )}
          </div>
        )}
      </div>

      {/* Sanitization Settings (Moltis-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Tool Output Sanitization</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={saveSanitizerMaxSize}
            disabled={sanitizerLoading}
          >
            {sanitizerLoading ? "Saving…" : "Save"}
          </button>
        </div>
        <p className="card-note">
          Sanitize tool output before feeding back to LLM. Strips secrets, base64, truncates large results.
        </p>
        <div className="settings-grid">
          <div>
            <label className="card-label" htmlFor="sanitizer-max">
              Max output size (bytes)
            </label>
            <input
              id="sanitizer-max"
              className="text-input"
              type="number"
              min="1024"
              max="1048576"
              value={sanitizerMaxSize}
              onChange={(event) => setSanitizerMaxSize(Number(event.target.value))}
            />
            <span className="card-note">Default: 51200 (50KB)</span>
          </div>
        </div>
      </div>

      {/* HTTP Allowlist (IronClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>HTTP Allowlist</h3>
          <label className="toggle-label">
            <input
              type="checkbox"
              checked={allowlistEnabled}
              onChange={(e) => toggleAllowlist(e.target.checked)}
              disabled={securityLoading}
            />
            <span>{allowlistEnabled ? "Enabled" : "Disabled"}</span>
          </label>
        </div>
        <p className="card-note">
          Restrict HTTP requests to approved domains only. Prevents tool exfiltration of credentials.
        </p>
        <div className="settings-grid">
          <div>
            <label className="card-label" htmlFor="new-domain">Add domain</label>
            <div className="input-group">
              <input
                id="new-domain"
                className="text-input"
                type="text"
                placeholder="example.com"
                value={newDomain}
                onChange={(e) => setNewDomain(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addAllowedDomain()}
              />
              <button type="button" onClick={addAllowedDomain} disabled={securityLoading || !newDomain.trim()}>
                Add
              </button>
            </div>
          </div>
        </div>
        {allowedDomains.length > 0 && (
          <div className="tag-list">
            {allowedDomains.map((domain) => (
              <span key={domain} className="tag">
                {domain}
                <button type="button" onClick={() => blockDomain(domain)} title="Block domain">
                  &times;
                </button>
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Leak Detection Test (IronClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Leak Detection</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={testLeakDetection}
            disabled={securityLoading}
          >
            {securityLoading ? "Testing…" : "Test Detection"}
          </button>
        </div>
        <p className="card-note">
          Scans tool output for potential credential leaks (API keys, tokens, passwords).
        </p>
        {leakTestResult && (
          <div className="result-box">
            <p><strong>Blocked:</strong> {leakTestResult.blocked ? "Yes" : "No"}</p>
            <p><strong>Matches found:</strong> {leakTestResult.matches?.length || 0}</p>
            {leakTestResult.matches?.length > 0 && (
              <ul>
                {leakTestResult.matches.map((match, i) => (
                  <li key={i} className="warning-text">
                    {match.pattern_name} ({match.severity})
                  </li>
                ))}
              </ul>
            )}
            {leakTestResult.error && (
              <p className="error-text">{leakTestResult.error}</p>
            )}
          </div>
        )}
      </div>

      {/* Tunnel (ZeroClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Tunnel</h3>
          <div className="button-row">
            <button
              type="button"
              onClick={refreshTunnel}
              disabled={securityLoading}
              className="ghost-button"
            >
              Refresh
            </button>
            {tunnelRunning ? (
              <button type="button" onClick={stopTunnel} className="danger-button">
                Stop
              </button>
            ) : (
              <button
                type="button"
                onClick={startTunnel}
                disabled={securityLoading || tunnelConfig.provider === "none"}
              >
                Start
              </button>
            )}
          </div>
        </div>
        <p className="card-note">
          Expose local server via Cloudflare, Tailscale, or ngrok tunnel.
        </p>
        <div className="settings-grid">
          <div>
            <label className="card-label">Provider</label>
            <select
              className="text-input"
              value={tunnelConfig.provider}
              onChange={(e) => setTunnelConfig((prev) => ({ ...prev, provider: e.target.value }))}
            >
              <option value="none">None</option>
              <option value="cloudflare">Cloudflare</option>
              <option value="tailscale">Tailscale</option>
              <option value="ngrok">ngrok</option>
            </select>
          </div>
          {tunnelConfig.provider === "cloudflare" && (
            <div>
              <label className="card-label">Cloudflare Token</label>
              <input
                className="text-input"
                type="password"
                placeholder="Your Cloudflare token"
                value={tunnelConfig.cloudflareToken}
                onChange={(e) => setTunnelConfig((prev) => ({ ...prev, cloudflareToken: e.target.value }))}
              />
            </div>
          )}
          {tunnelConfig.provider === "tailscale" && (
            <div>
              <label className="card-label">Tailscale Hostname</label>
              <input
                className="text-input"
                type="text"
                placeholder="your-hostname.tail-scale.ts.net"
                value={tunnelConfig.tailscaleHostname}
                onChange={(e) => setTunnelConfig((prev) => ({ ...prev, tailscaleHostname: e.target.value }))}
              />
            </div>
          )}
          {tunnelConfig.provider === "ngrok" && (
            <div>
              <label className="card-label">ngrok Token</label>
              <input
                className="text-input"
                type="password"
                placeholder="Your ngrok token"
                value={tunnelConfig.ngrokToken}
                onChange={(e) => setTunnelConfig((prev) => ({ ...prev, ngrokToken: e.target.value }))}
              />
            </div>
          )}
        </div>
        {tunnelRunning && tunnelUrl && (
          <div className="result-box">
            <p><strong>Tunnel URL:</strong> <a href={tunnelUrl} target="_blank" rel="noopener">{tunnelUrl}</a></p>
          </div>
        )}
      </div>

      {/* Observability (ZeroClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Observability</h3>
          <div className="button-row">
            <button
              type="button"
              onClick={refreshMetrics}
              disabled={observabilityLoading}
              className="ghost-button"
            >
              Refresh
            </button>
            <button
              type="button"
              onClick={resetMetrics}
              disabled={observabilityLoading}
              className="ghost-button"
            >
              Reset
            </button>
          </div>
        </div>
        <p className="card-note">
          View Prometheus metrics and OpenTelemetry traces.
        </p>
        {metrics && (
          <div className="settings-grid">
            <div><span className="card-label">Requests</span><span className="card-value">{metrics.requests_total}</span></div>
            <div><span className="card-label">AI Calls</span><span className="card-value">{metrics.ai_calls_total}</span></div>
            <div><span className="card-label">Actions</span><span className="card-value">{metrics.actions_executed}</span></div>
            <div><span className="card-label">Memory Entries</span><span className="card-value">{metrics.memory_entries}</span></div>
          </div>
        )}
        {traces.length > 0 && (
          <div className="result-box">
            <p><strong>Recent Traces:</strong></p>
            <ul>
              {traces.slice(0, 5).map((t) => (
                <li key={t.id}>
                  {t.name} - {t.status?.Error ? `Error: ${t.status.Error}` : "OK"}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
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
