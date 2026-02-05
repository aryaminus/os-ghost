import { useState, useEffect, useCallback } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";

const WorkflowsSection = ({ settingsState }) => {
  const [workflows, setWorkflows] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [recording, setRecording] = useState(false);
  const [recordingProgress, setRecordingProgress] = useState(null);
  const [selectedWorkflow, setSelectedWorkflow] = useState(null);
  const [replayStatus, setReplayStatus] = useState(null);
  const [newWorkflowForm, setNewWorkflowForm] = useState({
    name: "",
    description: "",
    startUrl: "",
  });
  const [showNewForm, setShowNewForm] = useState(false);

  // Load workflows on mount
  useEffect(() => {
    loadWorkflows();
  }, []);

  // Poll recording progress
  useEffect(() => {
    if (!recording) return;
    
    const interval = setInterval(async () => {
      try {
        const progress = await invoke("get_recording_progress");
        setRecordingProgress(progress);
      } catch (err) {
        console.error("Failed to get recording progress", err);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [recording]);

  const loadWorkflows = async () => {
    setLoading(true);
    try {
      const data = await invoke("get_all_workflows");
      setWorkflows(data);
    } catch (err) {
      console.error("Failed to load workflows", err);
      setError("Unable to load workflows");
    } finally {
      setLoading(false);
    }
  };

  const handleStartRecording = async () => {
    if (!newWorkflowForm.name || !newWorkflowForm.startUrl) return;
    
    try {
      await invoke("start_workflow_recording", {
        name: newWorkflowForm.name,
        description: newWorkflowForm.description,
        startUrl: newWorkflowForm.startUrl,
      });
      setRecording(true);
      setShowNewForm(false);
      setNewWorkflowForm({ name: "", description: "", startUrl: "" });
    } catch (err) {
      console.error("Failed to start recording", err);
      setError(typeof err === "string" ? err : "Failed to start recording");
    }
  };

  const handleStopRecording = async () => {
    try {
      const workflow = await invoke("stop_workflow_recording");
      setRecording(false);
      setRecordingProgress(null);
      await loadWorkflows();
      setSelectedWorkflow(workflow);
    } catch (err) {
      console.error("Failed to stop recording", err);
      setError(typeof err === "string" ? err : "Failed to stop recording");
    }
  };

  const handleCancelRecording = async () => {
    try {
      await invoke("cancel_workflow_recording");
      setRecording(false);
      setRecordingProgress(null);
    } catch (err) {
      console.error("Failed to cancel recording", err);
    }
  };

  const handleExecuteWorkflow = async (workflowId) => {
    setReplayStatus({ status: "running", progress: 0 });
    try {
      const result = await invoke("execute_workflow", {
        workflowId,
        autonomyLevel: settingsState.privacy?.autonomy_level || "supervised",
      });
      setReplayStatus({ 
        status: result.success ? "success" : "error", 
        result 
      });
      await loadWorkflows(); // Refresh to get updated stats
    } catch (err) {
      console.error("Failed to execute workflow", err);
      setReplayStatus({ 
        status: "error", 
        error: typeof err === "string" ? err : "Execution failed" 
      });
    }
  };

  const handleDeleteWorkflow = async (workflowId) => {
    if (!confirm("Are you sure you want to delete this workflow?")) return;
    
    try {
      await invoke("delete_workflow", { id: workflowId });
      await loadWorkflows();
      if (selectedWorkflow?.id === workflowId) {
        setSelectedWorkflow(null);
      }
    } catch (err) {
      console.error("Failed to delete workflow", err);
      setError("Failed to delete workflow");
    }
  };

  const formatDuration = (seconds) => {
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    return `${(seconds / 60).toFixed(1)}m`;
  };

  return (
    <section className="settings-section">
      <header className="section-header">
        <h2>Workflows</h2>
        <p>Record and replay repetitive browser tasks</p>
      </header>

      <div className="settings-card">
        <div className="card-row">
          <div className="card-label">Workflow Recording</div>
          <button
            type="button"
            className="primary-button"
            onClick={() => setShowNewForm(true)}
            disabled={recording}
          >
            {recording ? "Recording..." : "Record New Workflow"}
          </button>
        </div>

        {error && (
          <div className="alert alert-error">{error}</div>
        )}

        {/* Recording Interface */}
        {recording && (
          <div className="settings-card error">
            <div className="card-row">
              <div className="flex items-center gap-2">
                <span className="status-pill error">Recording</span>
                <span>Recording Workflow</span>
              </div>
              <div className="button-row">
                <button
                  type="button"
                  className="primary-button"
                  onClick={handleStopRecording}
                >
                  Stop Recording
                </button>
                <button
                  type="button"
                  className="ghost-button"
                  onClick={handleCancelRecording}
                >
                  Cancel
                </button>
              </div>
            </div>
            {recordingProgress && (
              <p className="card-note">
                Steps recorded: {recordingProgress.steps_recorded}
              </p>
            )}
            <p className="card-note">
              Perform the actions you want to record. Ghost will capture clicks,
              form fills, and navigation.
            </p>
          </div>
        )}

        {/* New Workflow Form */}
        {showNewForm && !recording && (
          <div className="settings-card">
            <h3>New Workflow</h3>
            <div className="form-group">
              <label>Name</label>
              <input
                type="text"
                value={newWorkflowForm.name}
                onChange={(e) =>
                  setNewWorkflowForm((prev) => ({ ...prev, name: e.target.value }))
                }
                placeholder="e.g., Book a flight"
                className="text-input"
              />
            </div>
            <div className="form-group">
              <label>Description</label>
              <input
                type="text"
                value={newWorkflowForm.description}
                onChange={(e) =>
                  setNewWorkflowForm((prev) => ({ ...prev, description: e.target.value }))
                }
                placeholder="What does this workflow do?"
                className="text-input"
              />
            </div>
            <div className="form-group">
              <label>Start URL</label>
              <input
                type="text"
                value={newWorkflowForm.startUrl}
                onChange={(e) =>
                  setNewWorkflowForm((prev) => ({ ...prev, startUrl: e.target.value }))
                }
                placeholder="https://example.com"
                className="text-input"
              />
            </div>
            <div className="button-row">
              <button
                type="button"
                className="primary-button"
                onClick={handleStartRecording}
                disabled={!newWorkflowForm.name || !newWorkflowForm.startUrl}
              >
                Start Recording
              </button>
              <button
                type="button"
                className="ghost-button"
                onClick={() => setShowNewForm(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* Workflow List */}
        {loading ? (
          <div className="settings-card subtle">
            <p className="card-note">Loading workflows...</p>
          </div>
        ) : workflows.length === 0 ? (
          <div className="settings-card subtle">
            <p className="card-label">No workflows yet</p>
            <p className="card-note">Record your first workflow to automate repetitive tasks</p>
          </div>
        ) : (
          <div className="list-grid">
            {workflows.map((workflow) => (
              <div
                key={workflow.id}
                className={`list-item ${
                  selectedWorkflow?.id === workflow.id ? "active" : ""
                }`}
                onClick={() => setSelectedWorkflow(workflow)}
              >
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <h4>{workflow.name}</h4>
                    {!workflow.enabled && (
                      <span className="status-pill warn">Disabled</span>
                    )}
                  </div>
                  <p className="card-note">
                    {workflow.description}
                  </p>
                  <div className="chip-grid">
                    <span className="chip">
                      {workflow.steps?.length || 0} steps
                    </span>
                    <span className="chip">
                      {workflow.execution_count || 0} runs
                    </span>
                    <span className={`chip ${workflow.success_rate >= 0.9 ? "active" : ""}`}>
                      {((workflow.success_rate || 0) * 100).toFixed(0)}% success
                    </span>
                    {workflow.avg_execution_time_secs > 0 && (
                      <span className="chip">
                        {formatDuration(workflow.avg_execution_time_secs)} avg
                      </span>
                    )}
                  </div>
                </div>
                <div className="button-row compact">
                  <button
                    type="button"
                    className="ghost-button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleExecuteWorkflow(workflow.id);
                    }}
                    disabled={!workflow.enabled || replayStatus?.status === "running"}
                  >
                    Run
                  </button>
                  <button
                    type="button"
                    className="ghost-button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteWorkflow(workflow.id);
                    }}
                  >
                    Delete
                  </button>
                </div>

                {/* Replay Status */}
                {replayStatus && selectedWorkflow?.id === workflow.id && (
                  <div className="note-editor">
                    {replayStatus.status === "running" && (
                      <div className="flex items-center gap-2">
                        <span className="status-pill">Executing...</span>
                      </div>
                    )}
                    {replayStatus.status === "success" && (
                      <div className="flex items-center gap-2">
                        <span className="status-pill ok">
                          Completed in {formatDuration(replayStatus.result?.duration_secs || 0)}
                        </span>
                      </div>
                    )}
                    {replayStatus.status === "error" && (
                      <div className="flex items-center gap-2">
                        <span className="status-pill error">
                          {replayStatus.error || "Execution failed"}
                        </span>
                      </div>
                    )}
                  </div>
                )}

                {/* Selected Workflow Details */}
                {selectedWorkflow?.id === workflow.id && (
                  <div className="note-editor">
                    <h4>Workflow Steps</h4>
                    <div className="note-grid">
                      {workflow.steps?.map((step, index) => (
                        <div
                          key={index}
                          className="note-card"
                        >
                          <div className="note-card-header">
                            <span className="note-meta">{index + 1}.</span>
                            <span className="note-body">{step.description}</span>
                            <span className="chip active">
                              {step.action_type?.type || "action"}
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {/* Info Box */}
        <div className="settings-card">
          <h4>About Workflows</h4>
          <ul className="card-note">
            <li>Record repetitive tasks by demonstration</li>
            <li>Ghost replays workflows with visual verification</li>
            <li>All steps are previewed based on your Autonomy Level</li>
            <li>Success rate and execution time tracked automatically</li>
          </ul>
        </div>
      </div>
    </section>
  );
};

WorkflowsSection.propTypes = {
  settingsState: PropTypes.object.isRequired,
};

export default WorkflowsSection;
