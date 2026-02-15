import PropTypes from "prop-types";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../../utils/data";

const ScheduledTasksSection = ({ settingsState, onSettingsUpdated }) => {
  const [tasks, setTasks] = useState([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [showAddForm, setShowAddForm] = useState(false);
  const [newTask, setNewTask] = useState({
    name: "",
    cron_expression: "",
    command: "",
  });
  const [message, setMessage] = useState("");

  const refreshTasks = useCallback(async () => {
    setLoading(true);
    try {
      const taskList = await safeInvoke("get_scheduled_tasks", {}, []);
      setTasks(Array.isArray(taskList) ? taskList : []);
    } catch (err) {
      console.error("Failed to load scheduled tasks", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshTasks();
  }, [refreshTasks]);

  const handleAddTask = useCallback(async () => {
    if (!newTask.name || !newTask.cron_expression || !newTask.command) {
      setMessage("Please fill in all fields");
      return;
    }
    setSaving(true);
    try {
      await invoke("add_scheduled_task", {
        name: newTask.name,
        cronExpression: newTask.cron_expression,
        command: newTask.command,
      });
      setNewTask({ name: "", cron_expression: "", command: "" });
      setShowAddForm(false);
      setMessage("Task added successfully");
      await refreshTasks();
      if (onSettingsUpdated) onSettingsUpdated();
    } catch (err) {
      console.error("Failed to add task", err);
      setMessage("Failed to add task");
    } finally {
      setSaving(false);
    }
  }, [newTask, refreshTasks, onSettingsUpdated]);

  const handleRemoveTask = useCallback(async (taskId) => {
    setSaving(true);
    try {
      await invoke("remove_scheduled_task", { taskId });
      setMessage("Task removed");
      await refreshTasks();
      if (onSettingsUpdated) onSettingsUpdated();
    } catch (err) {
      console.error("Failed to remove task", err);
      setMessage("Failed to remove task");
    } finally {
      setSaving(false);
    }
  }, [refreshTasks, onSettingsUpdated]);

  const handleToggleTask = useCallback(async (taskId, enabled) => {
    setSaving(true);
    try {
      await invoke("toggle_scheduled_task", { taskId, enabled });
      await refreshTasks();
      if (onSettingsUpdated) onSettingsUpdated();
    } catch (err) {
      console.error("Failed to toggle task", err);
      setMessage("Failed to toggle task");
    } finally {
      setSaving(false);
    }
  }, [refreshTasks, onSettingsUpdated]);

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Scheduled Tasks</h2>
        <p>Manage cron-based scheduled tasks (Moltis-inspired).</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <h3>Custom Tasks</h3>
          <button
            type="button"
            className="ghost-button"
            onClick={refreshTasks}
            disabled={loading}
          >
            {loading ? "Loading…" : "Refresh"}
          </button>
          <button
            type="button"
            className="ghost-button primary"
            onClick={() => setShowAddForm(!showAddForm)}
          >
            {showAddForm ? "Cancel" : "Add Task"}
          </button>
        </div>
        <p className="card-note">
          Define custom tasks that run on a schedule using cron expressions.
        </p>
      </div>

      {showAddForm && (
        <div className="settings-card">
          <h3>Add New Task</h3>
          <div className="settings-grid">
            <div>
              <label className="card-label" htmlFor="task-name">
                Task Name
              </label>
              <input
                id="task-name"
                className="text-input"
                type="text"
                value={newTask.name}
                onChange={(e) => setNewTask((prev) => ({ ...prev, name: e.target.value }))}
                placeholder="e.g., Daily backup"
              />
            </div>
            <div>
              <label className="card-label" htmlFor="task-cron">
                Cron Expression
              </label>
              <input
                id="task-cron"
                className="text-input"
                type="text"
                value={newTask.cron_expression}
                onChange={(e) => setNewTask((prev) => ({ ...prev, cron_expression: e.target.value }))}
                placeholder="e.g., 0 9 * * *"
              />
              <span className="card-note">Format: minute hour day month weekday</span>
            </div>
            <div>
              <label className="card-label" htmlFor="task-command">
                Command
              </label>
              <input
                id="task-command"
                className="text-input"
                type="text"
                value={newTask.command}
                onChange={(e) => setNewTask((prev) => ({ ...prev, command: e.target.value }))}
                placeholder="e.g., /path/to/script.sh"
              />
            </div>
          </div>
          <div className="button-row">
            <button
              type="button"
              className="ghost-button primary"
              onClick={handleAddTask}
              disabled={saving}
            >
              {saving ? "Adding…" : "Add Task"}
            </button>
          </div>
          {message && <p className="card-note">{message}</p>}
        </div>
      )}

      {tasks.length === 0 ? (
        <div className="settings-card">
          <p className="card-note">No custom scheduled tasks defined.</p>
        </div>
      ) : (
        <div className="settings-card">
          <h3>Task List</h3>
          <div className="list-grid">
            {tasks.map((task) => (
              <div key={task.id} className="list-item">
                <div>
                  <strong>{task.name}</strong>
                  <div className="card-note">
                    Cron: {task.cron_expression} | Command: {task.command}
                  </div>
                  {task.last_run && (
                    <div className="card-note">
                      Last run: {new Date(task.last_run * 1000).toLocaleString()}
                    </div>
                  )}
                </div>
                <div className="button-row compact">
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={task.enabled !== false}
                      onChange={(e) => handleToggleTask(task.id, e.target.checked)}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                  <button
                    type="button"
                    className="ghost-button danger"
                    onClick={() => handleRemoveTask(task.id)}
                    disabled={saving}
                  >
                    Remove
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="settings-card">
        <h3>Cron Reference</h3>
        <table className="data-table">
          <thead>
            <tr>
              <th>Field</th>
              <th>Values</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Minute</td>
              <td>0-59</td>
            </tr>
            <tr>
              <td>Hour</td>
              <td>0-23</td>
            </tr>
            <tr>
              <td>Day</td>
              <td>1-31</td>
            </tr>
            <tr>
              <td>Month</td>
              <td>1-12</td>
            </tr>
            <tr>
              <td>Weekday</td>
              <td>0-6 (Sun-Sat)</td>
            </tr>
          </tbody>
        </table>
        <p className="card-note">
          Examples: <code>0 9 * * *</code> = daily at 9am, <code>0 * * * *</code> = every hour
        </p>
      </div>
    </section>
  );
};

ScheduledTasksSection.propTypes = {
  settingsState: PropTypes.object,
  onSettingsUpdated: PropTypes.func,
};

export default ScheduledTasksSection;
