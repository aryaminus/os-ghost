/**
 * @fileoverview React hook for managing action confirmation, previews, and rollback.
 * Consolidates action-related state and Tauri IPC commands.
 * @module useActionManagement
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "../utils/data";

/**
 * Pending action requiring user confirmation
 * @typedef {Object} PendingAction
 * @property {number} id - Unique action ID
 * @property {string} action_type - Type of action
 * @property {string} description - Human-readable description
 * @property {string} target - Action target
 * @property {string} risk_level - Risk level (low, medium, high)
 * @property {string} status - Current status
 * @property {string} [reason] - Optional reason for action
 */

/**
 * Action preview state
 * @typedef {Object} ActionPreview
 * @property {string} id - Preview ID
 * @property {Object} action - The pending action
 * @property {string} state - Preview state (loading, streaming, ready, etc.)
 * @property {Object} [visual_preview] - Visual preview data
 * @property {number} progress - Streaming progress (0-1)
 * @property {Object} editable_params - Editable parameters
 * @property {boolean} is_reversible - Whether action can be undone
 * @property {string} [rollback_description] - Description of what undo does
 */

/**
 * Rollback status
 * @typedef {Object} RollbackStatus
 * @property {boolean} can_undo - Whether undo is available
 * @property {boolean} can_redo - Whether redo is available
 * @property {string} [undo_description] - Description of what will be undone
 * @property {string} [redo_description] - Description of what will be redone
 * @property {number} stack_size - Number of undoable actions
 */

/**
 * Sandbox configuration
 * @typedef {Object} SandboxConfig
 * @property {string} trust_level - Current trust level
 * @property {string[]} read_allowlist - Allowed read paths
 * @property {string[]} write_allowlist - Allowed write paths
 * @property {string[]} allowed_shell_categories - Enabled shell categories
 * @property {number} trust_score - Trust score (0-100)
 */

/**
 * Token usage statistics
 * @typedef {Object} TokenUsage
 * @property {number} gemini_calls - Gemini API call count
 * @property {number} ollama_calls - Ollama call count
 * @property {number} estimated_cost_usd - Estimated cost in USD
 */

/** Default rollback status */
const DEFAULT_ROLLBACK_STATUS = {
	can_undo: false,
	can_redo: false,
	undo_description: null,
	redo_description: null,
	stack_size: 0,
};

/**
 * Hook for managing action confirmation, previews, rollback, and sandbox.
 * Provides unified polling for agent status in active autonomy modes.
 *
 * @param {string} autonomyLevel - Current autonomy level
 * @param {boolean} apiKeyConfigured - Whether API key is configured
 * @returns {Object} Action management state and handlers
 */
export function useActionManagement(autonomyLevel = "observer", apiKeyConfigured = false) {
	// Pending actions requiring confirmation
	const [pendingActions, setPendingActions] = useState([]);
	// Action history for audit log
	const [actionHistory, setActionHistory] = useState([]);
	const [showActionHistory, setShowActionHistory] = useState(false);
	// Action preview state
	const [actionPreview, setActionPreview] = useState(null);
	// Rollback status
	const [rollbackStatus, setRollbackStatus] = useState(DEFAULT_ROLLBACK_STATUS);
	// Token usage
	const [tokenUsage, setTokenUsage] = useState({ gemini_calls: 0, ollama_calls: 0, estimated_cost_usd: 0 });
	// Model capabilities
	const [modelCapabilities, setModelCapabilities] = useState(null);
	// Sandbox settings
	const [sandboxSettings, setSandboxSettings] = useState(null);
	const [showSandboxSettings, setShowSandboxSettings] = useState(false);
	// Editing param state
	const [editingParam, setEditingParam] = useState(null);

	// Track if active mode (not observer)
	const isActiveMode = autonomyLevel && autonomyLevel !== "observer";

	// =========================================================================
	// Unified Polling (consolidates multiple frontend intervals)
	// =========================================================================
	useEffect(() => {
		if (!isActiveMode) {
			setPendingActions([]);
			setActionPreview(null);
			setRollbackStatus(DEFAULT_ROLLBACK_STATUS);
			return;
		}

		const pollAgentStatus = async () => {
			const status = await safeInvoke("poll_agent_status", {}, null);
			if (status) {
				// Update pending actions (except in autonomous mode)
				if (autonomyLevel !== "autonomous") {
					setPendingActions(status.pending_actions || []);
				}
				// Update action preview
				setActionPreview((prev) => {
					if (
						status.action_preview &&
						status.action_preview.state !== "completed" &&
						status.action_preview.state !== "cancelled"
					) {
						return status.action_preview;
					}
					return prev ? null : prev;
				});
				// Update rollback status
				if (status.rollback_status) {
					setRollbackStatus(status.rollback_status);
				}
				// Update token usage
				if (status.token_usage) {
					setTokenUsage(status.token_usage);
				}
			}
		};

		pollAgentStatus();
		const interval = setInterval(pollAgentStatus, 1500);
		return () => clearInterval(interval);
	}, [autonomyLevel, isActiveMode]);

	// =========================================================================
	// Model Capabilities
	// =========================================================================
	useEffect(() => {
		if (!apiKeyConfigured) return;
		const fetchCapabilities = async () => {
			const caps = await safeInvoke("get_model_capabilities", {}, null);
			if (caps) setModelCapabilities(caps);
		};
		fetchCapabilities();
	}, [apiKeyConfigured]);

	// =========================================================================
	// Observer Mode Token Usage (slower poll)
	// =========================================================================
	useEffect(() => {
		if (!apiKeyConfigured || isActiveMode) return;
		
		const fetchUsage = async () => {
			const usage = await safeInvoke("get_token_usage", {}, null);
			if (usage) setTokenUsage(usage);
		};
		fetchUsage();
		const interval = setInterval(fetchUsage, 30000);
		return () => clearInterval(interval);
	}, [apiKeyConfigured, isActiveMode]);

	// =========================================================================
	// Action Handlers
	// =========================================================================

	/** Approve a pending action */
	const approveAction = useCallback(async (actionId) => {
		if (actionPreview?.action?.id === actionId) {
			await safeInvoke("approve_preview", { preview_id: actionPreview.id }, null);
			setActionPreview(null);
			setPendingActions((prev) => prev.filter((a) => a.id !== actionId));
			return;
		}
		const result = await safeInvoke("approve_action", { action_id: actionId }, null);
		if (result) {
			await safeInvoke("execute_approved_action", { action_id: actionId }, null);
		}
		setPendingActions((prev) => prev.filter((a) => a.id !== actionId));
	}, [actionPreview]);

	/** Deny a pending action */
	const denyAction = useCallback(async (actionId) => {
		if (actionPreview?.action?.id === actionId) {
			await safeInvoke("deny_preview", { preview_id: actionPreview.id, reason: "User denied" }, null);
			setActionPreview(null);
		} else {
			await safeInvoke("deny_action", { action_id: actionId }, null);
		}
		setPendingActions((prev) => prev.filter((a) => a.id !== actionId));
	}, [actionPreview]);

	/** Approve the current preview */
	const approvePreview = useCallback(async () => {
		if (!actionPreview) return;
		await safeInvoke("approve_preview", { preview_id: actionPreview.id }, null);
		setActionPreview(null);
	}, [actionPreview]);

	/** Deny the current preview */
	const denyPreview = useCallback(async (reason) => {
		if (!actionPreview) return;
		await safeInvoke("deny_preview", { preview_id: actionPreview.id, reason }, null);
		setActionPreview(null);
	}, [actionPreview]);

	/** Edit a preview parameter */
	const editPreviewParam = useCallback(async (paramName, value) => {
		if (!actionPreview) return;
		const result = await safeInvoke(
			"update_preview_param",
			{ preview_id: actionPreview.id, param_name: paramName, value },
			null
		);
		if (result) {
			setActionPreview(result);
		}
		setEditingParam(null);
	}, [actionPreview]);

	/** Undo the last action */
	const undoAction = useCallback(async () => {
		const result = await safeInvoke("undo_action", {}, null);
		if (result?.success) {
			const status = await safeInvoke("get_rollback_status", {}, null);
			if (status) setRollbackStatus(status);
		}
		return result;
	}, []);

	/** Redo the last undone action */
	const redoAction = useCallback(async () => {
		const result = await safeInvoke("redo_action", {}, null);
		if (result?.success) {
			const status = await safeInvoke("get_rollback_status", {}, null);
			if (status) setRollbackStatus(status);
		}
		return result;
	}, []);

	/** Fetch action history */
	const fetchActionHistory = useCallback(async () => {
		const history = await safeInvoke("get_action_history", { limit: 50 }, []);
		setActionHistory(history || []);
		setShowActionHistory(true);
	}, []);

	/** Close action history modal */
	const closeActionHistory = useCallback(() => {
		setShowActionHistory(false);
	}, []);

	// =========================================================================
	// Sandbox Handlers
	// =========================================================================

	/** Fetch sandbox settings */
	const fetchSandboxSettings = useCallback(async () => {
		const settings = await safeInvoke("get_sandbox_settings", {}, null);
		if (settings) setSandboxSettings(settings);
	}, []);

	/** Open sandbox settings modal */
	const openSandboxSettings = useCallback(async () => {
		await fetchSandboxSettings();
		setShowSandboxSettings(true);
	}, [fetchSandboxSettings]);

	/** Close sandbox settings modal */
	const closeSandboxSettings = useCallback(() => {
		setShowSandboxSettings(false);
	}, []);

	/** Set sandbox trust level */
	const setTrustLevel = useCallback(async (level) => {
		const result = await safeInvoke("set_sandbox_trust_level", { level }, null);
		if (result) setSandboxSettings(result);
	}, []);

	/** Toggle shell category */
	const toggleShellCategory = useCallback(async (category, enabled) => {
		const cmd = enabled ? "enable_shell_category" : "disable_shell_category";
		const result = await safeInvoke(cmd, { category }, null);
		if (result) setSandboxSettings(result);
	}, []);

	/** Add read path to allowlist */
	const addReadPath = useCallback(async (path) => {
		const result = await safeInvoke("add_sandbox_read_path", { path }, null);
		if (result) {
			setSandboxSettings(result);
			return { success: true };
		}
		return { success: false, error: "Failed to add read path" };
	}, []);

	/** Remove read path from allowlist */
	const removeReadPath = useCallback(async (path) => {
		const result = await safeInvoke("remove_sandbox_read_path", { path }, null);
		if (result) {
			setSandboxSettings(result);
			return { success: true };
		}
		return { success: false, error: "Failed to remove read path" };
	}, []);

	/** Add write path to allowlist */
	const addWritePath = useCallback(async (path) => {
		const result = await safeInvoke("add_sandbox_write_path", { path }, null);
		if (result) {
			setSandboxSettings(result);
			return { success: true };
		}
		return { success: false, error: "Failed to add write path" };
	}, []);

	/** Remove write path from allowlist */
	const removeWritePath = useCallback(async (path) => {
		const result = await safeInvoke("remove_sandbox_write_path", { path }, null);
		if (result) {
			setSandboxSettings(result);
			return { success: true };
		}
		return { success: false, error: "Failed to remove write path" };
	}, []);

	return {
		// State
		pendingActions,
		actionPreview,
		rollbackStatus,
		tokenUsage,
		modelCapabilities,
		sandboxSettings,
		actionHistory,
		showActionHistory,
		showSandboxSettings,
		editingParam,
		
		// State setters (for UI interaction)
		setEditingParam,
		
		// Action handlers
		approveAction,
		denyAction,
		approvePreview,
		denyPreview,
		editPreviewParam,
		undoAction,
		redoAction,
		
		// History
		fetchActionHistory,
		closeActionHistory,
		
		// Sandbox
		fetchSandboxSettings,
		openSandboxSettings,
		closeSandboxSettings,
		setTrustLevel,
		toggleShellCategory,
		addReadPath,
		removeReadPath,
		addWritePath,
		removeWritePath,
	};
}

export default useActionManagement;
