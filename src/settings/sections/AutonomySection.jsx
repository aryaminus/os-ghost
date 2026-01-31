import { useCallback, useEffect, useMemo, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";

const AUTONOMY_OPTIONS = [
  {
    value: "observer",
    label: "Observer",
    description: "Watch only. No actions executed.",
  },
  {
    value: "suggester",
    label: "Suggester",
    description: "Proposes every action for confirmation.",
  },
  {
    value: "supervised",
    label: "Supervised",
    description: "Auto-executes safe actions; confirms risky ones.",
  },
  {
    value: "autonomous",
    label: "Autonomous",
    description: "Full control within guardrails.",
  },
];

const PREVIEW_POLICIES = [
  { value: "always", label: "Always preview" },
  { value: "high_risk", label: "Only high risk" },
  { value: "off", label: "Off" },
];

const AutonomySection = ({ settingsState, onSettingsUpdated }) => {
  const privacy = settingsState.privacy;
  const autonomySettings = settingsState.autonomySettings;
  const intelligentMode = settingsState.intelligentMode;
  const schedulerSettings = settingsState.schedulerSettings;

  const [autonomyLevel, setAutonomyLevel] = useState("observer");
  const [previewPolicy, setPreviewPolicy] = useState("always");
  const [autoPuzzle, setAutoPuzzle] = useState(true);
  const [intelligent, setIntelligent] = useState(false);
  const [reflection, setReflection] = useState(false);
  const [guardrails, setGuardrails] = useState(false);
  const [scheduleForm, setScheduleForm] = useState({
    dailyBrief: true,
    idleSuggestions: true,
    focusSummary: false,
    quietHoursEnabled: true,
    quietStart: "22:00",
    quietEnd: "07:00",
  });
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (privacy?.autonomy_level) {
      setAutonomyLevel(privacy.autonomy_level);
    }
    if (privacy?.preview_policy) {
      setPreviewPolicy(privacy.preview_policy);
    }
  }, [privacy?.autonomy_level, privacy?.preview_policy]);

  useEffect(() => {
    if (typeof autonomySettings?.auto_puzzle_from_companion === "boolean") {
      setAutoPuzzle(autonomySettings.auto_puzzle_from_companion);
    }
  }, [autonomySettings?.auto_puzzle_from_companion]);

  useEffect(() => {
    if (!intelligentMode) return;
    setIntelligent(!!intelligentMode.intelligent_mode);
    setReflection(!!intelligentMode.reflection);
    setGuardrails(!!intelligentMode.guardrails);
  }, [intelligentMode]);

  useEffect(() => {
    if (!schedulerSettings) return;
    setScheduleForm({
      dailyBrief: !!schedulerSettings.daily_brief_enabled,
      idleSuggestions: !!schedulerSettings.idle_suggestions_enabled,
      focusSummary: !!schedulerSettings.focus_summary_enabled,
      quietHoursEnabled: !!schedulerSettings.quiet_hours_enabled,
      quietStart: schedulerSettings.quiet_hours_start || "22:00",
      quietEnd: schedulerSettings.quiet_hours_end || "07:00",
    });
  }, [schedulerSettings]);

  const selectedDescription = useMemo(() => {
    return AUTONOMY_OPTIONS.find((opt) => opt.value === autonomyLevel)?.description;
  }, [autonomyLevel]);

  const handleSaveAutonomy = useCallback(async () => {
    if (!privacy) return;
    setSaving(true);
    setMessage("");
    try {
      await invoke("update_privacy_settings", {
        captureConsent: !!privacy.capture_consent,
        aiAnalysisConsent: !!privacy.ai_analysis_consent,
        privacyNoticeAcknowledged: !!privacy.privacy_notice_acknowledged,
        readOnlyMode: !!privacy.read_only_mode,
        autonomyLevel: autonomyLevel,
        redactPii: privacy.redact_pii !== false,
        previewPolicy: previewPolicy,
      });
      setMessage("Autonomy updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update autonomy", err);
      setMessage("Unable to update autonomy.");
    } finally {
      setSaving(false);
    }
  }, [autonomyLevel, privacy, onSettingsUpdated]);

  const handleAutoPuzzleToggle = useCallback(async () => {
    const next = !autoPuzzle;
    setAutoPuzzle(next);
    try {
      await invoke("set_autonomy_settings", {
        auto_puzzle_from_companion: next,
      });
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update auto puzzle", err);
    }
  }, [autoPuzzle, onSettingsUpdated]);

  const handleIntelligentToggle = useCallback(async () => {
    const next = !intelligent;
    setIntelligent(next);
    const updated = await invoke("set_intelligent_mode", { enabled: next });
    setReflection(!!updated.reflection);
    setGuardrails(!!updated.guardrails);
  }, [intelligent]);

  const handleReflectionToggle = useCallback(async () => {
    const next = !reflection;
    setReflection(next);
    await invoke("set_reflection_mode", { enabled: next });
  }, [reflection]);

  const handleGuardrailsToggle = useCallback(async () => {
    const next = !guardrails;
    setGuardrails(next);
    await invoke("set_guardrails_mode", { enabled: next });
  }, [guardrails]);

  const handleScheduleChange = (key) => (event) => {
    const value = event.target.type === "checkbox" ? event.target.checked : event.target.value;
    setScheduleForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSaveSchedule = useCallback(async () => {
    setMessage("");
    try {
      await invoke("update_scheduler_settings", {
        dailyBriefEnabled: scheduleForm.dailyBrief,
        idleSuggestionsEnabled: scheduleForm.idleSuggestions,
        focusSummaryEnabled: scheduleForm.focusSummary,
        quietHoursEnabled: scheduleForm.quietHoursEnabled,
        quietHoursStart: scheduleForm.quietStart,
        quietHoursEnd: scheduleForm.quietEnd,
      });
      setMessage("Scheduler updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update scheduler", err);
      setMessage("Unable to update scheduler.");
    }
  }, [scheduleForm, onSettingsUpdated]);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Autonomy</h2>
        <p>Control how OS Ghost acts on your behalf.</p>
      </header>

      <div className="settings-card">
        <h3>Autonomy level</h3>
        <select
          className="select-control"
          value={autonomyLevel}
          onChange={(event) => setAutonomyLevel(event.target.value)}
        >
          {AUTONOMY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <p className="card-note">{selectedDescription}</p>
        <div className="button-row">
          <button
            type="button"
            className="primary-button"
            onClick={handleSaveAutonomy}
            disabled={saving}
          >
            {saving ? "Saving…" : "Save autonomy"}
          </button>
          {message && <span className="status-pill neutral">{message}</span>}
        </div>
      </div>

      <div className="settings-card">
        <h3>Preview policy</h3>
        <select
          className="select-control"
          value={previewPolicy}
          onChange={(event) => setPreviewPolicy(event.target.value)}
        >
          {PREVIEW_POLICIES.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <p className="card-note">Controls when the system shows action previews.</p>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleSaveAutonomy}>
            Save preview policy
          </button>
        </div>
      </div>

      <div className="settings-card">
        <h3>Companion behaviors</h3>
        <label className="checkbox-row">
          <input type="checkbox" checked={autoPuzzle} onChange={handleAutoPuzzleToggle} />
          <span>Auto-create puzzles from companion observations.</span>
        </label>
      </div>

      <div className="settings-card">
        <h3>Intelligence stack</h3>
        <label className="checkbox-row">
          <input type="checkbox" checked={intelligent} onChange={handleIntelligentToggle} />
          <span>Enable planning pipeline.</span>
        </label>
        <label className="checkbox-row">
          <input type="checkbox" checked={reflection} onChange={handleReflectionToggle} />
          <span>Enable reflection (critic loop).</span>
        </label>
        <label className="checkbox-row">
          <input type="checkbox" checked={guardrails} onChange={handleGuardrailsToggle} />
          <span>Enable guardrails and security checks.</span>
        </label>
      </div>

      <div className="settings-card">
        <h3>Proactive routines</h3>
        <label className="checkbox-row">
          <input type="checkbox" checked={scheduleForm.dailyBrief} onChange={handleScheduleChange("dailyBrief")} />
          <span>Daily brief summary.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={scheduleForm.idleSuggestions}
            onChange={handleScheduleChange("idleSuggestions")}
          />
          <span>Idle-time suggestions.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={scheduleForm.focusSummary}
            onChange={handleScheduleChange("focusSummary")}
          />
          <span>Focus session summaries.</span>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={scheduleForm.quietHoursEnabled}
            onChange={handleScheduleChange("quietHoursEnabled")}
          />
          <span>Respect quiet hours.</span>
        </label>
        <div className="input-row">
          <input
            className="text-input"
            value={scheduleForm.quietStart}
            onChange={handleScheduleChange("quietStart")}
            placeholder="22:00"
          />
          <input
            className="text-input"
            value={scheduleForm.quietEnd}
            onChange={handleScheduleChange("quietEnd")}
            placeholder="07:00"
          />
        </div>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleSaveSchedule}>
            Save routines
          </button>
        </div>
      </div>
    </section>
  );
};

AutonomySection.propTypes = {
  settingsState: PropTypes.shape({
    privacy: PropTypes.object,
    autonomySettings: PropTypes.object,
    intelligentMode: PropTypes.object,
    schedulerSettings: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default AutonomySection;
