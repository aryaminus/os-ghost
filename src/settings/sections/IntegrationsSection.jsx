import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";

const IntegrationsSection = ({ settingsState, onSettingsUpdated }) => {
  const calendarSettings = settingsState.calendarSettings;
  const notes = settingsState.notes || [];
  const filesSettings = settingsState.filesSettings;
  const emailSettings = settingsState.emailSettings;

  const [calendarForm, setCalendarForm] = useState({
    enabled: false,
    icsPath: "",
    lookaheadDays: 7,
  });
  const [events, setEvents] = useState([]);
  const [loadingEvents, setLoadingEvents] = useState(false);
  const [noteDraft, setNoteDraft] = useState({ title: "", body: "" });
  const [editNoteId, setEditNoteId] = useState(null);
  const [filesForm, setFilesForm] = useState({ enabled: false, roots: "", maxResults: 10 });
  const [recentFiles, setRecentFiles] = useState([]);
  const [emailForm, setEmailForm] = useState({ enabled: false, provider: "none", inboxLimit: 10 });
  const [emailStatus, setEmailStatus] = useState({ connected: false, accountEmail: null, lastSyncAt: null });
  const [emailInbox, setEmailInbox] = useState([]);
  const [emailTriage, setEmailTriage] = useState([]);
  const [emailLoading, setEmailLoading] = useState(false);
  const [emailApplyLoading, setEmailApplyLoading] = useState(false);
  const [emailError, setEmailError] = useState("");
  const [persona, setPersona] = useState(null);
  const [personaDraft, setPersonaDraft] = useState(null);
  const [personaSaving, setPersonaSaving] = useState(false);
  const [notifications, setNotifications] = useState([]);
  const [notificationsLoading, setNotificationsLoading] = useState(false);
  const [systemNotificationsEnabled, setSystemNotificationsEnabled] = useState(true);

  // Channel state (ZeroClaw-inspired)
  const [availableChannels, setAvailableChannels] = useState([]);
  const [channelConfig, setChannelConfig] = useState({ telegram: "", discord: "", slack: "" });

  // AIEOS Identity state (ZeroClaw-inspired)
  const [identityData, setIdentityData] = useState(null);

  useEffect(() => {
    let mounted = true;
    const loadSettings = async () => {
      try {
        const settings = await invoke("get_notification_settings");
        if (mounted && settings) {
          setSystemNotificationsEnabled(!!settings.system_enabled);
        }
      } catch (err) {
        console.error("Failed to load notification settings", err);
      }
    };
    loadSettings();
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    if (!calendarSettings) return;
    setCalendarForm({
      enabled: !!calendarSettings.enabled,
      icsPath: calendarSettings.ics_path || "",
      lookaheadDays: calendarSettings.lookahead_days || 7,
    });
  }, [calendarSettings]);

  useEffect(() => {
    if (!filesSettings) return;
    setFilesForm({
      enabled: !!filesSettings.enabled,
      roots: (filesSettings.roots || []).join(", "),
      maxResults: filesSettings.max_results || 10,
    });
  }, [filesSettings]);

  useEffect(() => {
    if (!emailSettings) return;
    setEmailForm({
      enabled: !!emailSettings.enabled,
      provider: emailSettings.provider || "none",
      inboxLimit: emailSettings.inbox_limit || 10,
    });
    setEmailStatus({
      connected: !!emailSettings.connected,
      accountEmail: emailSettings.account_email || null,
      lastSyncAt: emailSettings.last_sync_at || null,
    });
  }, [emailSettings]);

  useEffect(() => {
    let mounted = true;
    const loadPersona = async () => {
      try {
        const profile = await invoke("get_persona");
        if (mounted && profile) {
          setPersona(profile);
          setPersonaDraft(profile);
        }
      } catch (err) {
        console.error("Failed to load persona", err);
      }
    };
    loadPersona();
    return () => {
      mounted = false;
    };
  }, []);

  const refreshNotifications = useCallback(async () => {
    setNotificationsLoading(true);
    try {
      const entries = await invoke("list_notifications", { limit: 20 });
      setNotifications(Array.isArray(entries) ? entries : []);
    } catch (err) {
      console.error("Failed to load notifications", err);
      setNotifications([]);
    } finally {
      setNotificationsLoading(false);
    }
  }, []);

  const clearNotifications = useCallback(async () => {
    try {
      await invoke("clear_notifications");
      setNotifications([]);
    } catch (err) {
      console.error("Failed to clear notifications", err);
    }
  }, []);

  const toggleSystemNotifications = useCallback(async () => {
    const next = !systemNotificationsEnabled;
    setSystemNotificationsEnabled(next);
    try {
      await invoke("set_notification_settings", { systemEnabled: next });
    } catch (err) {
      console.error("Failed to update notification settings", err);
    }
  }, [systemNotificationsEnabled]);

  // Channel handlers (ZeroClaw-inspired)
  const loadChannels = useCallback(async () => {
    try {
      const available = await invoke("get_available_channels");
      setAvailableChannels(Array.isArray(available) ? available : []);
    } catch (err) {
      console.error("Failed to load channels", err);
    }
  }, []);

  const saveChannel = useCallback(async (channel) => {
    try {
      // Store as secret - this would need to be implemented
      await invoke("secrets_store", { key: `channel_${channel}`, value: channelConfig[channel] });
    } catch (err) {
      console.error(`Failed to save ${channel} channel`, err);
    }
  }, [channelConfig]);

  // AIEOS Identity handlers (ZeroClaw-inspired)
  const loadIdentity = useCallback(async () => {
    try {
      const identity = await invoke("get_current_aieos_identity");
      setIdentityData(identity);
    } catch (err) {
      console.error("Failed to load identity", err);
      setIdentityData(null);
    }
  }, []);

  useEffect(() => {
    refreshNotifications();
    loadChannels();
    loadIdentity();
  }, [refreshNotifications, loadChannels, loadIdentity]);

  const refreshEvents = useCallback(async () => {
    setLoadingEvents(true);
    try {
      const upcoming = await invoke("get_upcoming_events", { limit: 10 });
      setEvents(Array.isArray(upcoming) ? upcoming : []);
    } catch (err) {
      console.error("Failed to load events", err);
      setEvents([]);
    } finally {
      setLoadingEvents(false);
    }
  }, []);

  useEffect(() => {
    if (calendarForm.enabled && calendarForm.icsPath) {
      refreshEvents();
    }
  }, [calendarForm.enabled, calendarForm.icsPath, refreshEvents]);

  const refreshFiles = useCallback(async () => {
    try {
      const results = await invoke("list_recent_files");
      setRecentFiles(Array.isArray(results) ? results : []);
    } catch (err) {
      console.error("Failed to load recent files", err);
      setRecentFiles([]);
    }
  }, []);

  const handleCalendarChange = (key) => (event) => {
    const value = event.target.type === "checkbox" ? event.target.checked : event.target.value;
    setCalendarForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSaveCalendar = useCallback(async () => {
    try {
      await invoke("update_calendar_settings", {
        enabled: calendarForm.enabled,
        icsPath: calendarForm.icsPath || null,
        lookaheadDays: Number(calendarForm.lookaheadDays) || 7,
      });
      onSettingsUpdated?.();
      refreshEvents();
    } catch (err) {
      console.error("Failed to save calendar settings", err);
    }
  }, [calendarForm, onSettingsUpdated, refreshEvents]);

  const handleSaveFiles = useCallback(async () => {
    try {
      const roots = filesForm.roots
        .split(",")
        .map((root) => root.trim())
        .filter(Boolean);
      await invoke("update_files_settings", {
        enabled: filesForm.enabled,
        roots,
        maxResults: Number(filesForm.maxResults) || 10,
      });
      onSettingsUpdated?.();
      refreshFiles();
    } catch (err) {
      console.error("Failed to save file settings", err);
    }
  }, [filesForm, onSettingsUpdated, refreshFiles]);

  const handleSaveEmail = useCallback(async () => {
    try {
      await invoke("update_email_settings", {
        enabled: emailForm.enabled,
        provider: emailForm.provider,
        inboxLimit: Number(emailForm.inboxLimit) || 10,
      });
      onSettingsUpdated?.();
    } catch (err) {
      console.error("Failed to save email settings", err);
    }
  }, [emailForm, onSettingsUpdated]);

  const handleEmailConnect = useCallback(async () => {
    setEmailError("");
    try {
      const next = await invoke("email_begin_oauth", { provider: emailForm.provider });
      if (next) {
        setEmailStatus({
          connected: !!next?.connected,
          accountEmail: next?.account_email || null,
          lastSyncAt: next?.last_sync_at || null,
        });
      }
      onSettingsUpdated?.();
    } catch (err) {
      console.error("Failed to connect email", err);
      setEmailError(
        err?.message || (typeof err === "string" ? err : "Email connection failed.")
      );
    }
  }, [emailForm.provider, onSettingsUpdated]);

  const handleEmailDisconnect = useCallback(async () => {
    setEmailError("");
    try {
      const next = await invoke("email_disconnect");
      setEmailStatus({
        connected: !!next?.connected,
        accountEmail: next?.account_email || null,
        lastSyncAt: next?.last_sync_at || null,
      });
      setEmailInbox([]);
      setEmailTriage([]);
      onSettingsUpdated?.();
    } catch (err) {
      console.error("Failed to disconnect email", err);
      setEmailError(
        err?.message || (typeof err === "string" ? err : "Email disconnect failed.")
      );
    }
  }, [onSettingsUpdated]);

  const refreshEmailInbox = useCallback(async () => {
    setEmailLoading(true);
    setEmailError("");
    try {
      const messages = await invoke("list_email_inbox", { limit: Number(emailForm.inboxLimit) || 10 });
      setEmailInbox(Array.isArray(messages) ? messages : []);
    } catch (err) {
      console.error("Failed to load inbox", err);
      setEmailInbox([]);
      setEmailError(
        err?.message || (typeof err === "string" ? err : "Failed to load inbox.")
      );
    } finally {
      setEmailLoading(false);
    }
  }, [emailForm.inboxLimit]);

  const runEmailTriage = useCallback(async () => {
    setEmailLoading(true);
    setEmailError("");
    try {
      const decisions = await invoke("triage_email_inbox", { limit: Number(emailForm.inboxLimit) || 10 });
      setEmailTriage(Array.isArray(decisions) ? decisions : []);
    } catch (err) {
      console.error("Failed to triage inbox", err);
      setEmailTriage([]);
      setEmailError(
        err?.message || (typeof err === "string" ? err : "Failed to triage inbox.")
      );
    } finally {
      setEmailLoading(false);
    }
  }, [emailForm.inboxLimit]);

  const applyEmailTriage = useCallback(async () => {
    setEmailApplyLoading(true);
    setEmailError("");
    try {
      await invoke("apply_email_triage", { decisions: emailTriage });
      setEmailTriage([]);
      await refreshEmailInbox();
    } catch (err) {
      console.error("Failed to apply triage", err);
      setEmailError(
        err?.message || (typeof err === "string" ? err : "Failed to apply triage.")
      );
    } finally {
      setEmailApplyLoading(false);
    }
  }, [emailTriage, refreshEmailInbox]);

  const handlePersonaSave = useCallback(async () => {
    if (!personaDraft) return;
    setPersonaSaving(true);
    try {
      const updated = await invoke("set_persona", { profile: personaDraft });
      if (updated) {
        setPersona(updated);
        setPersonaDraft(updated);
      }
      onSettingsUpdated?.();
    } catch (err) {
      console.error("Failed to save persona", err);
    } finally {
      setPersonaSaving(false);
    }
  }, [personaDraft, onSettingsUpdated]);

  const handleNoteSave = useCallback(async () => {
    if (!noteDraft.title.trim() && !noteDraft.body.trim()) return;
    try {
      if (editNoteId) {
        const existing = notes.find((note) => note.id === editNoteId);
        if (!existing) return;
        await invoke("update_note", {
          note: { ...existing, title: noteDraft.title, body: noteDraft.body },
        });
      } else {
        await invoke("add_note", { title: noteDraft.title, body: noteDraft.body });
      }
      setNoteDraft({ title: "", body: "" });
      setEditNoteId(null);
      onSettingsUpdated?.();
    } catch (err) {
      console.error("Failed to save note", err);
    }
  }, [noteDraft, editNoteId, notes, onSettingsUpdated]);

  const handleEditNote = useCallback((note) => {
    setEditNoteId(note.id);
    setNoteDraft({ title: note.title, body: note.body });
  }, []);

  const handleDeleteNote = useCallback(
    async (id) => {
      try {
        await invoke("delete_note", { id });
        if (editNoteId === id) {
          setEditNoteId(null);
          setNoteDraft({ title: "", body: "" });
        }
        onSettingsUpdated?.();
      } catch (err) {
        console.error("Failed to delete note", err);
      }
    },
    [editNoteId, onSettingsUpdated]
  );

  const noteSummary = useMemo(() => {
    if (!notes.length) return "No notes yet.";
    return `${notes.length} notes saved.`;
  }, [notes.length]);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Integrations</h2>
        <p>Local-first calendar and notes support.</p>
      </header>

      <div className="settings-card">
        <h3>Calendar (ICS)</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={calendarForm.enabled}
            onChange={handleCalendarChange("enabled")}
          />
          <span>Enable local calendar sync.</span>
        </label>
        <div className="input-row">
          <input
            className="text-input"
            value={calendarForm.icsPath}
            onChange={handleCalendarChange("icsPath")}
            placeholder="/path/to/calendar.ics"
          />
        </div>
        <div className="input-row">
          <input
            className="text-input"
            type="number"
            min="1"
            max="30"
            value={calendarForm.lookaheadDays}
            onChange={handleCalendarChange("lookaheadDays")}
          />
          <span className="card-note">Lookahead days</span>
        </div>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleSaveCalendar}>
            Save calendar settings
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={refreshEvents}
            disabled={loadingEvents}
          >
            {loadingEvents ? "Loading…" : "Refresh events"}
          </button>
        </div>
        {events.length === 0 ? (
          <p className="card-note">
            No upcoming events found. Recurring events and timezone rules are excluded.
          </p>
        ) : (
          <div className="list-grid">
            {events.map((event) => (
              <div key={event.id} className="list-item">
                <div>
                  <strong>{event.summary}</strong>
                  <div className="card-note">
                    {new Date(event.starts_at * 1000).toLocaleString()}
                    {event.location ? ` · ${event.location}` : ""}
                  </div>
                </div>
                <span className="status-pill neutral">{event.all_day ? "All day" : "Event"}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>Notes</h3>
        <p className="card-note">{noteSummary}</p>
        <p className="card-note">Notes are stored locally on this device.</p>
        <div className="note-editor">
          <input
            className="text-input"
            value={noteDraft.title}
            onChange={(event) => setNoteDraft((prev) => ({ ...prev, title: event.target.value }))}
            placeholder="Title"
          />
          <textarea
            className="text-area"
            value={noteDraft.body}
            onChange={(event) => setNoteDraft((prev) => ({ ...prev, body: event.target.value }))}
            placeholder="Note details"
            rows={4}
          />
          <div className="note-editor-actions">
            <button type="button" className="ghost-button" onClick={handleNoteSave}>
              {editNoteId ? "Update note" : "Add note"}
            </button>
            {editNoteId && (
              <button
                type="button"
                className="ghost-button"
                onClick={() => {
                  setEditNoteId(null);
                  setNoteDraft({ title: "", body: "" });
                }}
              >
                Cancel edit
              </button>
            )}
          </div>
        </div>
        {notes.length === 0 ? (
          <p className="card-note">No notes saved yet.</p>
        ) : (
          <div className="note-grid">
            {notes.map((note) => (
              <div key={note.id} className="note-card">
                <div className="note-card-header">
                  <strong>{note.title || "(Untitled)"}</strong>
                  <span className="note-meta">
                    {new Date(note.updated_at * 1000).toLocaleDateString()}
                  </span>
                </div>
                <div className="note-body">{note.body.slice(0, 160)}</div>
                <div className="note-actions">
                  <button type="button" className="ghost-button" onClick={() => handleEditNote(note)}>
                    Edit
                  </button>
                  <button type="button" className="ghost-button" onClick={() => handleDeleteNote(note.id)}>
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>Recent files</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={filesForm.enabled}
            onChange={(event) =>
              setFilesForm((prev) => ({ ...prev, enabled: event.target.checked }))
            }
            placeholder="/Users/name/Documents, /Users/name/Downloads"
          />
          <span>Enable local file listing.</span>
        </label>
        <div className="input-row">
          <input
            className="text-input"
            value={filesForm.roots}
            onChange={(event) =>
              setFilesForm((prev) => ({ ...prev, roots: event.target.value }))
            }
            placeholder="/Users/name/Documents, /Users/name/Downloads"
          />
        </div>
        <div className="input-row">
          <input
            className="text-input"
            type="number"
            min="1"
            max="50"
            value={filesForm.maxResults}
            onChange={(event) =>
              setFilesForm((prev) => ({ ...prev, maxResults: event.target.value }))
            }
          />
          <span className="card-note">Max results</span>
        </div>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleSaveFiles}>
            Save file settings
          </button>
          <button type="button" className="ghost-button" onClick={refreshFiles}>
            Refresh files
          </button>
        </div>
        {recentFiles.length === 0 ? (
          <p className="card-note">No files found.</p>
        ) : (
          <div className="list-grid">
            {recentFiles.map((file) => (
              <div key={file.path} className="list-item">
                <div>
                  <strong>{file.name}</strong>
                  <div className="card-note">{file.path}</div>
                </div>
                <span className="status-pill neutral">
                  {file.modified_at
                    ? new Date(file.modified_at * 1000).toLocaleDateString()
                    : "Unknown"}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Email</span>
          <span className={`status-pill ${emailStatus.connected ? "ok" : "neutral"}`}>
            {emailStatus.connected ? "Connected" : "Not connected"}
          </span>
        </div>
        {emailStatus.accountEmail && (
          <div className="card-row">
            <span className="card-label">Account</span>
            <span className="card-value">{emailStatus.accountEmail}</span>
          </div>
        )}
        {emailStatus.lastSyncAt && (
          <div className="card-row">
            <span className="card-label">Last sync</span>
            <span className="card-value">
              {new Date(emailStatus.lastSyncAt * 1000).toLocaleTimeString()}
            </span>
          </div>
        )}
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={emailForm.enabled}
            onChange={(event) =>
              setEmailForm((prev) => ({ ...prev, enabled: event.target.checked }))
            }
          />
          <span>Enable email integration.</span>
        </label>
        <div className="input-row">
          <select
            className="text-input"
            value={emailForm.provider}
            onChange={(event) =>
              setEmailForm((prev) => ({ ...prev, provider: event.target.value }))
            }
          >
            <option value="none">None</option>
            <option value="gmail">Gmail</option>
            <option value="imap">IMAP</option>
          </select>
          <span className="card-note">Provider</span>
        </div>
        <div className="input-row">
          <input
            className="text-input"
            type="number"
            min="1"
            max="50"
            value={emailForm.inboxLimit}
            onChange={(event) =>
              setEmailForm((prev) => ({ ...prev, inboxLimit: event.target.value }))
            }
          />
          <span className="card-note">Inbox limit</span>
        </div>
        <div className="button-row">
          <button type="button" className="ghost-button" onClick={handleSaveEmail}>
            Save email settings
          </button>
          {!emailStatus.connected ? (
            <button
              type="button"
              className="primary-button"
              onClick={handleEmailConnect}
              disabled={!emailForm.enabled || emailForm.provider === "none"}
            >
              Connect
            </button>
          ) : (
            <button type="button" className="ghost-button" onClick={handleEmailDisconnect}>
              Disconnect
            </button>
          )}
        </div>
        {emailError && <p className="card-note">{emailError}</p>}
        <div className="button-row">
          <button
            type="button"
            className="ghost-button"
            onClick={refreshEmailInbox}
            disabled={!emailStatus.connected || emailLoading}
          >
            {emailLoading ? "Loading…" : "Load inbox"}
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={runEmailTriage}
            disabled={!emailStatus.connected || emailLoading}
          >
            {emailLoading ? "Working…" : "Run triage"}
          </button>
        </div>
        {!emailStatus.connected ? (
          <p className="card-note">Connect an account to load inbox data.</p>
        ) : emailInbox.length === 0 ? (
          <p className="card-note">No messages loaded yet.</p>
        ) : (
          <div className="list-grid">
            {emailInbox.map((msg) => (
              <div key={msg.id} className="list-item">
                <div>
                  <strong>{msg.subject}</strong>
                  <div className="card-note">{msg.from}</div>
                  <div className="card-note">{msg.snippet}</div>
                </div>
                <span className={`status-pill ${msg.is_unread ? "warn" : "neutral"}`}>
                  {msg.is_unread ? "Unread" : "Read"}
                </span>
              </div>
            ))}
          </div>
        )}
        {emailTriage.length > 0 && (
          <>
            <div className="button-row">
              <button
                type="button"
                className="ghost-button"
                onClick={applyEmailTriage}
                disabled={emailApplyLoading}
              >
                {emailApplyLoading ? "Applying…" : "Apply triage"}
              </button>
            </div>
            <div className="list-grid">
              {emailTriage.map((decision) => (
                <div key={decision.message_id} className="list-item">
                  <div>
                    <strong>{decision.action}</strong>
                    <div className="card-note">{decision.summary}</div>
                  </div>
                  <span className="status-pill neutral">
                    {Math.round(decision.confidence * 100)}%
                  </span>
                </div>
              ))}
            </div>
          </>
        )}
      </div>

      <div className="settings-card">
        <h3>Persona</h3>
        {!personaDraft ? (
          <p className="card-note">Persona unavailable.</p>
        ) : (
          <div className="note-editor">
            <input
              className="text-input"
              value={personaDraft.name}
              onChange={(event) =>
                setPersonaDraft((prev) => ({ ...prev, name: event.target.value }))
              }
              placeholder="Name"
            />
            <input
              className="text-input"
              value={personaDraft.description}
              onChange={(event) =>
                setPersonaDraft((prev) => ({ ...prev, description: event.target.value }))
              }
              placeholder="Description"
            />
            <input
              className="text-input"
              value={personaDraft.tone}
              onChange={(event) =>
                setPersonaDraft((prev) => ({ ...prev, tone: event.target.value }))
              }
              placeholder="Tone"
            />
            <div className="input-row">
              <input
                className="text-input"
                type="number"
                min="0"
                max="1"
                step="0.1"
                value={personaDraft.hint_density}
                onChange={(event) =>
                  setPersonaDraft((prev) => ({
                    ...prev,
                    hint_density: Number(event.target.value),
                  }))
                }
              />
              <span className="card-note">Hint density (0-1)</span>
            </div>
            <div className="input-row">
              <input
                className="text-input"
                type="number"
                min="0"
                max="1"
                step="0.1"
                value={personaDraft.action_aggressiveness}
                onChange={(event) =>
                  setPersonaDraft((prev) => ({
                    ...prev,
                    action_aggressiveness: Number(event.target.value),
                  }))
                }
              />
              <span className="card-note">Action aggressiveness (0-1)</span>
            </div>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={!!personaDraft.allow_auto_intents}
                onChange={(event) =>
                  setPersonaDraft((prev) => ({
                    ...prev,
                    allow_auto_intents: event.target.checked,
                  }))
                }
              />
              <span>Allow auto intents</span>
            </label>
            <div className="button-row">
              <button
                type="button"
                className="ghost-button"
                onClick={handlePersonaSave}
                disabled={personaSaving}
              >
                {personaSaving ? "Saving…" : "Save persona"}
              </button>
              <button
                type="button"
                className="ghost-button"
                onClick={() => setPersonaDraft(persona)}
              >
                Reset
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="settings-card">
        <h3>Notifications</h3>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={systemNotificationsEnabled}
            onChange={toggleSystemNotifications}
          />
          <span>Show system notifications.</span>
        </label>
        <div className="button-row">
          <button
            type="button"
            className="ghost-button"
            onClick={refreshNotifications}
            disabled={notificationsLoading}
          >
            {notificationsLoading ? "Loading…" : "Refresh"}
          </button>
          <button type="button" className="ghost-button" onClick={clearNotifications}>
            Clear
          </button>
        </div>
        {notifications.length === 0 ? (
          <p className="card-note">No notifications.</p>
        ) : (
          <div className="list-grid">
            {notifications.map((note) => (
              <div key={note.id} className="list-item">
                <div>
                  <strong>{note.title}</strong>
                  <div className="card-note">{note.body}</div>
                </div>
                <span className="status-pill neutral">
                  {new Date(note.timestamp * 1000).toLocaleTimeString()}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Channels (ZeroClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>Channels</h3>
          <button type="button" className="ghost-button" onClick={loadChannels}>
            Refresh
          </button>
        </div>
        <p className="card-note">
          Connect via Telegram, Discord, or Slack for multi-channel messaging.
        </p>
        
        <div className="settings-grid">
          <div>
            <label className="card-label">Telegram</label>
            <div className="input-group">
              <input
                className="text-input"
                type="text"
                placeholder="Bot token"
                value={channelConfig.telegram}
                onChange={(e) => setChannelConfig((prev) => ({ ...prev, telegram: e.target.value }))}
              />
              <button type="button" onClick={() => saveChannel("telegram")} disabled={!channelConfig.telegram.trim()}>
                Save
              </button>
            </div>
          </div>
          <div>
            <label className="card-label">Discord</label>
            <div className="input-group">
              <input
                className="text-input"
                type="text"
                placeholder="Bot token"
                value={channelConfig.discord}
                onChange={(e) => setChannelConfig((prev) => ({ ...prev, discord: e.target.value }))}
              />
              <button type="button" onClick={() => saveChannel("discord")} disabled={!channelConfig.discord.trim()}>
                Save
              </button>
            </div>
          </div>
          <div>
            <label className="card-label">Slack</label>
            <div className="input-group">
              <input
                className="text-input"
                type="text"
                placeholder="Bot token"
                value={channelConfig.slack}
                onChange={(e) => setChannelConfig((prev) => ({ ...prev, slack: e.target.value }))}
              />
              <button type="button" onClick={() => saveChannel("slack")} disabled={!channelConfig.slack.trim()}>
                Save
              </button>
            </div>
          </div>
        </div>

        {availableChannels.length > 0 && (
          <div className="card-row">
            <span className="card-label">Available:</span>
            <span className="card-value">{availableChannels.join(", ")}</span>
          </div>
        )}
      </div>

      {/* AIEOS Identity (ZeroClaw-inspired) */}
      <div className="settings-card">
        <div className="card-row">
          <h3>AI Identity</h3>
          <button type="button" className="ghost-button" onClick={loadIdentity}>
            Reload
          </button>
        </div>
        <p className="card-note">
          Define the AI persona using AIEOS (AI Entity Object Specification).
        </p>
        
        {identityData ? (
          <div className="settings-grid">
            <div>
              <span className="card-label">Name</span>
              <span className="card-value">{identityData.identity?.name || "Unnamed"}</span>
            </div>
            <div>
              <span className="card-label">Bio</span>
              <span className="card-value">{identityData.identity?.bio || "No bio"}</span>
            </div>
            <div>
              <span className="card-label">Traits</span>
              <span className="card-value">
                {identityData.psychology?.traits?.join(", ") || "None"}
              </span>
            </div>
          </div>
        ) : (
          <p className="card-note">No identity configured. Create identity.json in data directory.</p>
        )}
      </div>

    </section>
  );
};

IntegrationsSection.propTypes = {
  settingsState: PropTypes.shape({
    calendarSettings: PropTypes.object,
    notes: PropTypes.array,
    filesSettings: PropTypes.object,
    emailSettings: PropTypes.object,
  }).isRequired,
  onSettingsUpdated: PropTypes.func,
};

export default IntegrationsSection;
