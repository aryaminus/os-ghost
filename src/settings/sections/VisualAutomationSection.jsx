import { useState, useEffect, useCallback } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";

const VisualAutomationSection = ({ settingsState, onSettingsUpdated }) => {
  const [formState, setFormState] = useState({
    visualAutomationConsent: false,
    visualAutomationAllowlist: [],
    visualAutomationBlocklist: [],
    maxVisualActionsPerMinute: 10,
    confirmNewSites: true,
  });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [newSite, setNewSite] = useState("");

  useEffect(() => {
    if (!settingsState.privacy) return;
    setFormState({
      visualAutomationConsent: !!settingsState.privacy.visual_automation_consent,
      visualAutomationAllowlist: settingsState.privacy.visual_automation_allowlist || [],
      visualAutomationBlocklist: settingsState.privacy.visual_automation_blocklist || [],
      maxVisualActionsPerMinute: settingsState.privacy.max_visual_actions_per_minute || 10,
      confirmNewSites: settingsState.privacy.confirm_new_sites !== false,
    });
  }, [settingsState.privacy]);

  const handleChange = (key) => (event) => {
    const value = event.target.type === "checkbox" 
      ? event.target.checked 
      : Number(event.target.value);
    setFormState((prev) => ({ ...prev, [key]: value }));
  };

  const handleAddToAllowlist = () => {
    if (!newSite.trim()) return;
    setFormState((prev) => ({
      ...prev,
      visualAutomationAllowlist: [...prev.visualAutomationAllowlist, newSite.trim()],
    }));
    setNewSite("");
  };

  const handleRemoveFromAllowlist = (site) => {
    setFormState((prev) => ({
      ...prev,
      visualAutomationAllowlist: prev.visualAutomationAllowlist.filter((s) => s !== site),
    }));
  };

  const handleAddToBlocklist = () => {
    if (!newSite.trim()) return;
    setFormState((prev) => ({
      ...prev,
      visualAutomationBlocklist: [...prev.visualAutomationBlocklist, newSite.trim()],
    }));
    setNewSite("");
  };

  const handleRemoveFromBlocklist = (site) => {
    setFormState((prev) => ({
      ...prev,
      visualAutomationBlocklist: prev.visualAutomationBlocklist.filter((s) => s !== site),
    }));
  };

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError("");
    setSuccess("");
    try {
      await invoke("update_privacy_settings", {
        ...settingsState.privacy,
        visualAutomationConsent: formState.visualAutomationConsent,
        visualAutomationAllowlist: formState.visualAutomationAllowlist,
        visualAutomationBlocklist: formState.visualAutomationBlocklist,
        maxVisualActionsPerMinute: formState.maxVisualActionsPerMinute,
        confirmNewSites: formState.confirmNewSites,
      });
      setSuccess("Visual automation settings updated.");
      onSettingsUpdated();
    } catch (err) {
      console.error("Failed to update visual automation settings", err);
      setError(typeof err === "string" ? err : "Unable to save settings.");
    } finally {
      setSaving(false);
    }
  }, [formState, onSettingsUpdated, settingsState.privacy]);

  return (
    <div className="settings-section">
      <h2 className="settings-heading">Visual Automation</h2>
      <p className="settings-description">
        Configure AI-powered visual automation for browser interactions.
      </p>

      {error && (
        <div className="alert alert-error">{error}</div>
      )}
      {success && (
        <div className="alert alert-success">{success}</div>
      )}

      <div className="settings-form">
        {/* Main Consent Toggle */}
        <div className="form-group">
          <label className="form-label">
            <input
              type="checkbox"
              checked={formState.visualAutomationConsent}
              onChange={handleChange("visualAutomationConsent")}
              className="form-checkbox"
            />
            <span>Enable Visual Automation</span>
          </label>
          <p className="form-help">
            Allow Ghost to see and interact with browser elements
          </p>
        </div>

        {formState.visualAutomationConsent && (
          <>
            {/* Rate Limiting */}
            <div className="form-group">
              <label className="form-label">Rate Limiting</label>
              <div className="form-row">
                <label>Max actions per minute:</label>
                <input
                  type="number"
                  min="1"
                  max="60"
                  value={formState.maxVisualActionsPerMinute}
                  onChange={handleChange("maxVisualActionsPerMinute")}
                  className="form-input number-input"
                />
              </div>
              <p className="form-help">
                Limits how many automated actions Ghost can perform per minute
              </p>
            </div>

            {/* Confirm New Sites */}
            <div className="form-group">
              <label className="form-label">
                <input
                  type="checkbox"
                  checked={formState.confirmNewSites}
                  onChange={handleChange("confirmNewSites")}
                  className="form-checkbox"
                />
                <span>Confirm New Sites</span>
              </label>
              <p className="form-help">
                Always ask for confirmation on sites not in allowlist
              </p>
            </div>

            {/* Site Management */}
            <div className="form-group">
              <label className="form-label">Site Management</label>
              <div className="form-row">
                <input
                  type="text"
                  value={newSite}
                  onChange={(e) => setNewSite(e.target.value)}
                  placeholder="example.com"
                  className="form-input"
                />
                <button
                  type="button"
                  onClick={handleAddToAllowlist}
                  className="ghost-button"
                >
                  Add to Allowlist
                </button>
                <button
                  type="button"
                  onClick={handleAddToBlocklist}
                  className="ghost-button danger"
                >
                  Add to Blocklist
                </button>
              </div>

              {/* Allowlist */}
              {formState.visualAutomationAllowlist.length > 0 && (
                <div className="site-list">
                  <h4>Allowed Sites</h4>
                  <p className="form-help">
                    Automation only works on these sites (if not empty)
                  </p>
                  <div className="tag-list">
                    {formState.visualAutomationAllowlist.map((site) => (
                      <span key={site} className="tag tag-success">
                        {site}
                        <button
                          onClick={() => handleRemoveFromAllowlist(site)}
                          className="tag-remove"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Blocklist */}
              {formState.visualAutomationBlocklist.length > 0 && (
                <div className="site-list">
                  <h4>Blocked Sites</h4>
                  <p className="form-help">
                    Automation never works on these sites
                  </p>
                  <div className="tag-list">
                    {formState.visualAutomationBlocklist.map((site) => (
                      <span key={site} className="tag tag-danger">
                        {site}
                        <button
                          onClick={() => handleRemoveFromBlocklist(site)}
                          className="tag-remove"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </>
        )}

        {/* Save Button */}
        <div className="form-actions">
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="ghost-button primary"
          >
            {saving ? "Saving..." : "Save Visual Automation Settings"}
          </button>
        </div>
      </div>

      {/* Info Box */}
      <div className="info-box">
        <h4>About Visual Automation</h4>
        <ul>
          <li>Ghost uses AI vision to identify buttons, links, and form fields</li>
          <li>All actions are previewed before execution based on your Autonomy Level</li>
          <li>Form values are masked in logs for privacy</li>
          <li>Visual automation works only with your explicit consent</li>
        </ul>
      </div>
    </div>
  );
};

VisualAutomationSection.propTypes = {
  settingsState: PropTypes.object.isRequired,
  onSettingsUpdated: PropTypes.func.isRequired,
};

export default VisualAutomationSection;
