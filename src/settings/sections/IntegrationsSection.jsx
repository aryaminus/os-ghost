import { useCallback, useEffect, useMemo, useState } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";

const IntegrationsSection = ({ settingsState, onSettingsUpdated }) => {
  const calendarSettings = settingsState.calendarSettings;
  const notes = settingsState.notes || [];

  const [calendarForm, setCalendarForm] = useState({
    enabled: false,
    icsPath: "",
    lookaheadDays: 7,
  });
  const [events, setEvents] = useState([]);
  const [loadingEvents, setLoadingEvents] = useState(false);
  const [noteDraft, setNoteDraft] = useState({ title: "", body: "" });
  const [editNoteId, setEditNoteId] = useState(null);

  useEffect(() => {
    if (!calendarSettings) return;
    setCalendarForm({
      enabled: !!calendarSettings.enabled,
      icsPath: calendarSettings.ics_path || "",
      lookaheadDays: calendarSettings.lookahead_days || 7,
    });
  }, [calendarSettings]);

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
            No upcoming events found. Recurring events and timezone rules are not parsed yet.
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
        <div className="card-row">
          <span className="card-label">Email</span>
          <span className="status-pill neutral">Planned</span>
        </div>
        <p className="card-note">
          OAuth-based inbox triage with strict consent and redaction.
        </p>
      </div>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Remote access</span>
          <span className="status-pill neutral">Planned</span>
        </div>
        <p className="card-note">
          Secure, opt-in remote control similar to Tailscale workflows.
        </p>
      </div>

      <div className="settings-card">
        <div className="card-row">
          <span className="card-label">Multi-channel inbox</span>
          <span className="status-pill neutral">Planned</span>
        </div>
        <p className="card-note">
          Unified channel routing for chat, SMS, and work messaging.
        </p>
      </div>
    </section>
  );
};

IntegrationsSection.propTypes = {
  settingsState: PropTypes.shape({
    calendarSettings: PropTypes.object,
    notes: PropTypes.array,
  }).isRequired,
  onSettingsUpdated: PropTypes.func,
};

export default IntegrationsSection;
