import { useCallback, useEffect, useMemo, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";

const PrivacySection = ({ settingsState, onSettingsUpdated }) => {
  const [formState, setFormState] = useState({
    captureConsent: false,
    aiConsent: false,
    noticeAck: false,
    readOnly: false,
    redactPii: true,
  });
  const [saving, setSaving] = useState(false);
  const [monitorSaving, setMonitorSaving] = useState(false);
  const [captureSaving, setCaptureSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [monitorMessage, setMonitorMessage] = useState("");
  const [captureMessage, setCaptureMessage] = useState("");
  const permissionDiagnostics = settingsState.permissionDiagnostics;

  const [monitorForm, setMonitorForm] = useState({
    enabled: true,
    intervalSecs: 60,
    idleSecs: 900,
    allowHidden: false,
    onlyCompanion: true,
    recentCount: 5,
    idleStreak: 3,
    categoryWindow: 10,
  });

  const [captureFormat, setCaptureFormat] = useState("jpeg");

  useEffect(() => {
    if (!settingsState.privacy) return;
    setFormState({
      captureConsent: !!settingsState.privacy.capture_consent,
      aiConsent: !!settingsState.privacy.ai_analysis_consent,
      noticeAck: !!settingsState.privacy.privacy_notice_acknowledged,
      readOnly: !!settingsState.privacy.read_only_mode,
      redactPii: settingsState.privacy.redact_pii !== false,
    });
  }, [settingsState.privacy]);

  useEffect(() => {
    if (!settingsState.systemSettings) return;
    setMonitorForm({
      enabled: settingsState.systemSettings.monitor_enabled ?? true,
      intervalSecs: settingsState.systemSettings.monitor_interval_secs,
      idleSecs: settingsState.systemSettings.monitor_idle_secs,
      allowHidden: settingsState.systemSettings.monitor_allow_hidden,
      onlyCompanion: settingsState.systemSettings.monitor_only_companion,
      recentCount: settingsState.systemSettings.monitor_recent_activity_count,
      idleStreak: settingsState.systemSettings.monitor_idle_streak_threshold,
      categoryWindow: settingsState.systemSettings.monitor_category_window,
    });
  }, [settingsState.systemSettings]);

  useEffect(() => {
    if (!settingsState.captureSettings) return;
    setCaptureFormat(settingsState.captureSettings.image_format || "jpeg");
  }, [settingsState.captureSettings]);

  const consentStatus = useMemo(() => {
    if (!settingsState.privacy) return "Unknown";
    if (settingsState.privacy.read_only_mode) return "Read-only";
    if (
      settingsState.privacy.capture_consent &&
      settingsState.privacy.ai_analysis_consent &&
      settingsState.privacy.privacy_notice_acknowledged
    ) {
      return "Granted";
    }
    return "Required";
  }, [settingsState.privacy]);

  const handleChange = (key) => (event) => {
    const value = event.target.checked;
    setFormState((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = useCallback(async () => {
    if (!settingsState.privacy) return;
    setSaving(true);
    setError("");
    setSuccess("");
    try {
      await invoke("update_privacy_settings", {
        captureConsent: formState.captureConsent,
        aiAnalysisConsent: formState.aiConsent,
        privacyNoticeAcknowledged: formState.noticeAck,
        readOnlyMode: formState.readOnly,
        autonomyLevel: settingsState.privacy.autonomy_level || "autonomous",
        redactPii: formState.redactPii,
        previewPolicy: settingsState.privacy.preview_policy || "always",
      });
      setSuccess("Privacy settings updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update privacy settings", err);
      setError(typeof err === "string" ? err : "Unable to save privacy settings.");
    } finally {
      setSaving(false);
    }
  }, [formState, onSettingsUpdated, settingsState.privacy]);

  const notice = settingsState.privacyNotice || "Privacy notice unavailable.";

  const handleReset = useCallback(async () => {
    const current = await safeInvoke("get_privacy_settings", {}, null);
    if (current) {
      setFormState({
        captureConsent: !!current.capture_consent,
        aiConsent: !!current.ai_analysis_consent,
        noticeAck: !!current.privacy_notice_acknowledged,
        readOnly: !!current.read_only_mode,
        redactPii: current.redact_pii !== false,
      });
      setSuccess("");
      setError("");
    }
  }, []);

  const handleMonitorChange = (key) => (event) => {
    const value = event.target.type === "checkbox" ? event.target.checked : Number(event.target.value);
    setMonitorForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSaveMonitoring = useCallback(async () => {
    if (!settingsState.systemSettings) return;
    setMonitorSaving(true);
    setMonitorMessage("");
    try {
      await invoke("update_system_settings", {
        monitorEnabled: monitorForm.enabled,
        monitorIntervalSecs: monitorForm.intervalSecs,
        monitorIdleSecs: monitorForm.idleSecs,
        monitorAllowHidden: monitorForm.allowHidden,
        monitorOnlyCompanion: monitorForm.onlyCompanion,
        monitorRecentActivityCount: monitorForm.recentCount,
        monitorIdleStreakThreshold: monitorForm.idleStreak,
        monitorCategoryWindow: monitorForm.categoryWindow,
        globalShortcutEnabled: settingsState.systemSettings.global_shortcut_enabled,
      });
      setMonitorMessage("Monitoring settings updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update monitoring settings", err);
      setMonitorMessage("Unable to update monitoring settings.");
    } finally {
      setMonitorSaving(false);
    }
  }, [monitorForm, onSettingsUpdated, settingsState.systemSettings]);

  const handleSaveCapture = useCallback(async () => {
    setCaptureSaving(true);
    setCaptureMessage("");
    try {
      await invoke("set_capture_settings", { imageFormat: captureFormat });
      setCaptureMessage("Capture settings updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update capture settings", err);
      setCaptureMessage("Unable to update capture settings.");
    } finally {
      setCaptureSaving(false);
    }
  }, [captureFormat, onSettingsUpdated]);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Privacy & Security</h2>
        <p>Consent and safety controls for monitoring and analysis.</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Consent status</span>
          <span className={`status-pill ${consentStatus === "Granted" ? "ok" : "warn"}`}>
            {consentStatus}
          </span>
        </div>
        {settingsState.privacy?.consent_timestamp && (
          <div className="card-row">
            <span className="card-label">Last updated</span>
            <span className="card-value">
              {new Date(settingsState.privacy.consent_timestamp * 1000).toLocaleString()}
            </span>
          </div>
        )}
        <p className="card-note">
          Monitoring remains paused until consent is granted.
        </p>
      </div>

      <div className="settings-card">
        <h3>Permission diagnostics</h3>
        <div className="card-row">
          <span className="card-label">Screen recording</span>
          <span className={`status-pill ${permissionDiagnostics?.screen_recording?.status === "granted" ? "ok" : "warn"}`}>
            {permissionDiagnostics?.screen_recording?.status || "unknown"}
          </span>
        </div>
        <p className="card-note">
          {permissionDiagnostics?.screen_recording?.message || "Run a capture to verify permissions."}
        </p>
        {permissionDiagnostics?.screen_recording?.action_url && (
          <button
            type="button"
            className="ghost-button"
            onClick={() =>
              invoke("open_external_url", {
                url: permissionDiagnostics.screen_recording.action_url,
              })
            }
          >
            Open System Settings
          </button>
        )}
        <div className="card-row">
          <span className="card-label">Accessibility</span>
          <span className="status-pill neutral">
            {permissionDiagnostics?.accessibility?.status || "unknown"}
          </span>
        </div>
        {permissionDiagnostics?.accessibility?.action_url && (
          <button
            type="button"
            className="ghost-button"
            onClick={() =>
              invoke("open_external_url", {
                url: permissionDiagnostics.accessibility.action_url,
              })
            }
          >
            Open Accessibility Settings
          </button>
        )}
        <div className="card-row">
          <span className="card-label">Input monitoring</span>
          <span className="status-pill neutral">
            {permissionDiagnostics?.input_monitoring?.status || "unknown"}
          </span>
        </div>
        {permissionDiagnostics?.input_monitoring?.action_url && (
          <button
            type="button"
            className="ghost-button"
            onClick={() =>
              invoke("open_external_url", {
                url: permissionDiagnostics.input_monitoring.action_url,
              })
            }
          >
            Open Input Monitoring Settings
          </button>
        )}
      </div>

      <div className="settings-card">
        <h3>Consent controls</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={formState.noticeAck}
            onChange={handleChange("noticeAck")}
          />
          <span>I understand the privacy notice.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={formState.captureConsent}
            onChange={handleChange("captureConsent")}
          />
          <span>Allow screen capture for context gathering.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={formState.aiConsent}
            onChange={handleChange("aiConsent")}
          />
          <span>Allow AI analysis of captured data.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={formState.redactPii}
            onChange={handleChange("redactPii")}
          />
          <span>Redact sensitive information before analysis.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={formState.readOnly}
            onChange={handleChange("readOnly")}
          />
          <span>Enable read-only mode (no automation or capture).</span>
        </label>

        <div className="button-row">
          <button
            type="button"
            className="primary-button"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "Saving…" : "Apply changes"}
          </button>
          <button type="button" className="ghost-button" onClick={handleReset}>
            Reset
          </button>
          {success && <span className="status-pill ok">{success}</span>}
          {error && <span className="status-pill error">{error}</span>}
        </div>
      </div>

        <div className="settings-card">
        <h3>Monitoring cadence</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={monitorForm.enabled}
            onChange={handleMonitorChange("enabled")}
          />
          <span>Enable background monitoring.</span>
        </label>
        <div className="input-row">
          <label className="card-label" htmlFor="monitor-interval">
            Capture interval (seconds)
          </label>
          <input
            id="monitor-interval"
            className="text-input"
            type="number"
            min="10"
            max="3600"
            value={monitorForm.intervalSecs}
            onChange={handleMonitorChange("intervalSecs")}
          />
        </div>
        <div className="input-row">
          <label className="card-label" htmlFor="monitor-idle">
            Idle pause threshold (seconds)
          </label>
          <input
            id="monitor-idle"
            className="text-input"
            type="number"
            min="60"
            max="43200"
            value={monitorForm.idleSecs}
            onChange={handleMonitorChange("idleSecs")}
          />
        </div>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={monitorForm.allowHidden}
            onChange={handleMonitorChange("allowHidden")}
          />
          <span>Continue monitoring when Ghost window is hidden.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={monitorForm.onlyCompanion}
            onChange={handleMonitorChange("onlyCompanion")}
          />
          <span>Run monitoring only in companion mode.</span>
        </label>
        <div className="settings-grid">
          <div>
            <label className="card-label" htmlFor="monitor-recent">
              Recent activity count
            </label>
            <input
              id="monitor-recent"
              className="text-input"
              type="number"
              min="1"
              max="20"
              value={monitorForm.recentCount}
              onChange={handleMonitorChange("recentCount")}
            />
          </div>
          <div>
            <label className="card-label" htmlFor="monitor-idle-streak">
              Idle streak trigger
            </label>
            <input
              id="monitor-idle-streak"
              className="text-input"
              type="number"
              min="1"
              max="10"
              value={monitorForm.idleStreak}
              onChange={handleMonitorChange("idleStreak")}
            />
          </div>
          <div>
            <label className="card-label" htmlFor="monitor-window">
              Category window size
            </label>
            <input
              id="monitor-window"
              className="text-input"
              type="number"
              min="5"
              max="30"
              value={monitorForm.categoryWindow}
              onChange={handleMonitorChange("categoryWindow")}
            />
          </div>
        </div>
        <div className="button-row">
          <button
            type="button"
            className="primary-button"
            onClick={handleSaveMonitoring}
            disabled={monitorSaving}
          >
            {monitorSaving ? "Saving…" : "Apply monitoring"}
          </button>
          {monitorMessage && <span className="status-pill neutral">{monitorMessage}</span>}
        </div>
      </div>

      <div className="settings-card">
        <h3>Capture format</h3>
        <p className="card-note">JPEG is faster; PNG preserves detail.</p>
        <select
          className="select-control"
          value={captureFormat}
          onChange={(event) => setCaptureFormat(event.target.value)}
        >
          <option value="jpeg">JPEG (fast)</option>
          <option value="png">PNG (quality)</option>
        </select>
        <div className="button-row">
          <button
            type="button"
            className="ghost-button"
            onClick={handleSaveCapture}
            disabled={captureSaving}
          >
            {captureSaving ? "Saving…" : "Save capture format"}
          </button>
          {captureMessage && <span className="status-pill neutral">{captureMessage}</span>}
        </div>
      </div>

      <div className="settings-card">
        <h3>Privacy notice</h3>
        <pre className="notice-box">{notice}</pre>
      </div>
    </section>
  );
};

PrivacySection.propTypes = {
  settingsState: PropTypes.shape({
    privacy: PropTypes.object,
    privacyNotice: PropTypes.string,
    systemSettings: PropTypes.object,
    captureSettings: PropTypes.object,
    permissionDiagnostics: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default PrivacySection;
