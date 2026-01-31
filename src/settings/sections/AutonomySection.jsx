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

const AutonomySection = ({ settingsState, onSettingsUpdated }) => {
  const privacy = settingsState.privacy;
  const autonomySettings = settingsState.autonomySettings;
  const intelligentMode = settingsState.intelligentMode;

  const [autonomyLevel, setAutonomyLevel] = useState("autonomous");
  const [autoPuzzle, setAutoPuzzle] = useState(true);
  const [intelligent, setIntelligent] = useState(false);
  const [reflection, setReflection] = useState(false);
  const [guardrails, setGuardrails] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (privacy?.autonomy_level) {
      setAutonomyLevel(privacy.autonomy_level);
    }
  }, [privacy?.autonomy_level]);

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
    </section>
  );
};

AutonomySection.propTypes = {
  settingsState: PropTypes.shape({
    privacy: PropTypes.object,
    autonomySettings: PropTypes.object,
    intelligentMode: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default AutonomySection;
