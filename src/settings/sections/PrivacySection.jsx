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
    trustProfile: "balanced",
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
    ignoreIdle: false,
    allowHidden: false,
    onlyCompanion: true,
    recentCount: 5,
    idleStreak: 3,
    categoryWindow: 10,
    adaptiveEnabled: true,
    adaptiveMinIntervalSecs: 10,
    adaptiveMaxIntervalSecs: 300,
    adaptiveIdleThresholdSecs: 300,
    adaptiveLowActivityThresholdSecs: 60,
    adaptiveHighActivityCount: 20,
    changeDetectionEnabled: true,
    changePixelThreshold: 30,
    changeMinChangedPercentage: 0.01,
    changeMaxChangedPercentage: 0.95,
    analysisCooldownSecs: 90,
  });

  const PERFORMANCE_PROFILES = [
    { value: "low", label: "Low", cooldown: 180 },
    { value: "balanced", label: "Balanced", cooldown: 90 },
    { value: "high", label: "High", cooldown: 45 },
  ];

  const [captureFormat, setCaptureFormat] = useState("jpeg");

  useEffect(() => {
    if (!settingsState.privacy) return;
    setFormState({
      captureConsent: !!settingsState.privacy.capture_consent,
      aiConsent: !!settingsState.privacy.ai_analysis_consent,
      noticeAck: !!settingsState.privacy.privacy_notice_acknowledged,
      readOnly: !!settingsState.privacy.read_only_mode,
      redactPii: settingsState.privacy.redact_pii !== false,
      trustProfile: settingsState.privacy.trust_profile || "balanced",
    });
  }, [settingsState.privacy]);

  useEffect(() => {
    if (!settingsState.systemSettings) return;
    setMonitorForm({
      enabled: settingsState.systemSettings.monitor_enabled ?? true,
      intervalSecs: settingsState.systemSettings.monitor_interval_secs,
      idleSecs: settingsState.systemSettings.monitor_idle_secs,
      ignoreIdle: settingsState.systemSettings.monitor_ignore_idle ?? false,
      allowHidden: settingsState.systemSettings.monitor_allow_hidden,
      onlyCompanion: settingsState.systemSettings.monitor_only_companion,
      recentCount: settingsState.systemSettings.monitor_recent_activity_count,
      idleStreak: settingsState.systemSettings.monitor_idle_streak_threshold,
      categoryWindow: settingsState.systemSettings.monitor_category_window,
      adaptiveEnabled: settingsState.systemSettings.adaptive_capture_enabled ?? true,
      adaptiveMinIntervalSecs: settingsState.systemSettings.adaptive_min_interval_secs,
      adaptiveMaxIntervalSecs: settingsState.systemSettings.adaptive_max_interval_secs,
      adaptiveIdleThresholdSecs: settingsState.systemSettings.adaptive_idle_threshold_secs,
      adaptiveLowActivityThresholdSecs: settingsState.systemSettings.adaptive_low_activity_threshold_secs,
      adaptiveHighActivityCount: settingsState.systemSettings.adaptive_high_activity_count,
      changeDetectionEnabled: settingsState.systemSettings.change_detection_enabled ?? true,
      changePixelThreshold: settingsState.systemSettings.change_pixel_threshold,
      changeMinChangedPercentage: settingsState.systemSettings.change_min_changed_percentage,
      changeMaxChangedPercentage: settingsState.systemSettings.change_max_changed_percentage,
      analysisCooldownSecs: settingsState.systemSettings.analysis_cooldown_secs ?? 90,
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
        trustProfile: formState.trustProfile,
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

  const handlePerformanceProfile = (event) => {
    const profile = PERFORMANCE_PROFILES.find((p) => p.value === event.target.value);
    if (!profile) return;
    setMonitorForm((prev) => ({ ...prev, analysisCooldownSecs: profile.cooldown }));
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
        monitorIgnoreIdle: monitorForm.ignoreIdle,
        monitorAllowHidden: monitorForm.allowHidden,
        monitorOnlyCompanion: monitorForm.onlyCompanion,
        monitorRecentActivityCount: monitorForm.recentCount,
        monitorIdleStreakThreshold: monitorForm.idleStreak,
        monitorCategoryWindow: monitorForm.categoryWindow,
        globalShortcutEnabled: settingsState.systemSettings.global_shortcut_enabled,
        adaptiveCaptureEnabled: monitorForm.adaptiveEnabled,
        adaptiveMinIntervalSecs: monitorForm.adaptiveMinIntervalSecs,
        adaptiveMaxIntervalSecs: monitorForm.adaptiveMaxIntervalSecs,
        adaptiveIdleThresholdSecs: monitorForm.adaptiveIdleThresholdSecs,
        adaptiveLowActivityThresholdSecs: monitorForm.adaptiveLowActivityThresholdSecs,
        adaptiveHighActivityCount: monitorForm.adaptiveHighActivityCount,
        changeDetectionEnabled: monitorForm.changeDetectionEnabled,
        changePixelThreshold: monitorForm.changePixelThreshold,
        changeMinChangedPercentage: monitorForm.changeMinChangedPercentage,
        changeMaxChangedPercentage: monitorForm.changeMaxChangedPercentage,
        analysisCooldownSecs: monitorForm.analysisCooldownSecs,
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
        <div className="form-row">
          <label className="card-label" htmlFor="privacy-trust-profile">
            Trust profile
          </label>
          <select
            id="privacy-trust-profile"
            className="text-input"
            value={formState.trustProfile}
            onChange={(event) =>
              setFormState((prev) => ({ ...prev, trustProfile: event.target.value }))
            }
          >
            <option value="strict">Strict</option>
            <option value="balanced">Balanced</option>
            <option value="open">Open</option>
          </select>
        </div>

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
            checked={monitorForm.ignoreIdle}
            onChange={handleMonitorChange("ignoreIdle")}
          />
          <span>Continue monitoring even when idle.</span>
        </label>
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
            <label className="card-label" htmlFor="performance-profile">
              Performance budget
            </label>
            <select id="performance-profile" className="text-input" onChange={handlePerformanceProfile}>
              {PERFORMANCE_PROFILES.map((profile) => (
                <option key={profile.value} value={profile.value}>
                  {profile.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="card-label" htmlFor="monitor-analysis-cooldown">
              Analysis cooldown (sec)
            </label>
            <input
              id="monitor-analysis-cooldown"
              className="text-input"
              type="number"
              min="30"
              max="3600"
              value={monitorForm.analysisCooldownSecs}
              onChange={handleMonitorChange("analysisCooldownSecs")}
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
        <h3>Adaptive capture</h3>
        <p className="card-note">
          Dynamically adjusts capture intervals based on your activity level for more efficient monitoring.
        </p>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={monitorForm.adaptiveEnabled}
            onChange={handleMonitorChange("adaptiveEnabled")}
          />
          <span>Enable adaptive capture intervals.</span>
        </label>
        {monitorForm.adaptiveEnabled && (
          <div className="settings-grid">
            <div>
              <label className="card-label" htmlFor="adaptive-min-interval">
                Min interval (sec)
              </label>
              <input
                id="adaptive-min-interval"
                className="text-input"
                type="number"
                min="5"
                max="60"
                value={monitorForm.adaptiveMinIntervalSecs}
                onChange={handleMonitorChange("adaptiveMinIntervalSecs")}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="adaptive-max-interval">
                Max interval (sec)
              </label>
              <input
                id="adaptive-max-interval"
                className="text-input"
                type="number"
                min="60"
                max="3600"
                value={monitorForm.adaptiveMaxIntervalSecs}
                onChange={handleMonitorChange("adaptiveMaxIntervalSecs")}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="adaptive-idle-threshold">
                Idle threshold (sec)
              </label>
              <input
                id="adaptive-idle-threshold"
                className="text-input"
                type="number"
                min="30"
                max="3600"
                value={monitorForm.adaptiveIdleThresholdSecs}
                onChange={handleMonitorChange("adaptiveIdleThresholdSecs")}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="adaptive-low-activity">
                Low activity (sec)
              </label>
              <input
                id="adaptive-low-activity"
                className="text-input"
                type="number"
                min="10"
                max="300"
                value={monitorForm.adaptiveLowActivityThresholdSecs}
                onChange={handleMonitorChange("adaptiveLowActivityThresholdSecs")}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="adaptive-high-activity">
                High activity count
              </label>
              <input
                id="adaptive-high-activity"
                className="text-input"
                type="number"
                min="5"
                max="100"
                value={monitorForm.adaptiveHighActivityCount}
                onChange={handleMonitorChange("adaptiveHighActivityCount")}
              />
            </div>
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>Change detection</h3>
        <p className="card-note">
          Skip captures when screen content hasn't changed significantly.
        </p>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={monitorForm.changeDetectionEnabled}
            onChange={handleMonitorChange("changeDetectionEnabled")}
          />
          <span>Enable screen change detection.</span>
        </label>
        {monitorForm.changeDetectionEnabled && (
          <div className="settings-grid">
            <div>
              <label className="card-label" htmlFor="change-pixel-threshold">
                Pixel threshold (0-255)
              </label>
              <input
                id="change-pixel-threshold"
                className="text-input"
                type="number"
                min="0"
                max="255"
                value={monitorForm.changePixelThreshold}
                onChange={handleMonitorChange("changePixelThreshold")}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="change-min-percentage">
                Min changed (%)
              </label>
              <input
                id="change-min-percentage"
                className="text-input"
                type="number"
                min="0"
                max="100"
                step="0.01"
                value={(monitorForm.changeMinChangedPercentage * 100).toFixed(2)}
                onChange={(e) => handleMonitorChange("changeMinChangedPercentage")({
                  target: { value: Number(e.target.value) / 100 }
                })}
              />
            </div>
            <div>
              <label className="card-label" htmlFor="change-max-percentage">
                Max changed (%)
              </label>
              <input
                id="change-max-percentage"
                className="text-input"
                type="number"
                min="0"
                max="100"
                step="0.01"
                value={(monitorForm.changeMaxChangedPercentage * 100).toFixed(2)}
                onChange={(e) => handleMonitorChange("changeMaxChangedPercentage")({
                  target: { value: Number(e.target.value) / 100 }
                })}
              />
            </div>
          </div>
        )}
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
