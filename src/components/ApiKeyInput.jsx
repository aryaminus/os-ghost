/**
 * @fileoverview API Key and AI Provider configuration component.
 * Supports both Gemini API (cloud) and Ollama (local) AI providers.
 * @module ApiKeyInput
 */

import React, { useState, useCallback, useRef, useEffect } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../utils/data";

/** Default Ollama status when unavailable */
const DEFAULT_OLLAMA_STATUS = {
	ollama_available: false,
	active_provider: "None",
};

/** Initial Ollama state object */
const INITIAL_OLLAMA_STATE = Object.freeze({
	expanded: false,
	url: "",
	visionModel: "",
	textModel: "",
	defaults: {},
	status: null,
	error: "",
	success: "",
	saving: false,
});

/**
 * Validates a URL string format.
 * @param {string} url - URL to validate
 * @returns {boolean} True if valid URL format
 */
const isValidUrl = (url) => {
	if (!url) return false;
	try {
		new URL(url);
		return true;
	} catch {
		return false;
	}
};

/**
 * Unified AI configuration component.
 * Manages Gemini API key and Ollama local LLM settings.
 *
 * @param {Object} props - Component props
 * @param {function} [props.onKeySet] - Callback when configuration is successfully set
 * @param {string} [props.apiKeySource="none"] - Source of current key ("user", "env", "none")
 * @returns {JSX.Element} AI configuration form
 */
const ApiKeyInput = ({ onKeySet, apiKeySource = "none" }) => {
	// Gemini state (kept separate - small, independent concerns)
	const [apiKey, setApiKey] = useState("");
	const [error, setError] = useState("");
	const [isLoading, setIsLoading] = useState(false);
	const [showKey, setShowKey] = useState(false);
	const inputRef = useRef(null);

	// Ollama state (grouped - related concerns)
	const [ollama, setOllama] = useState(INITIAL_OLLAMA_STATE);

	// Refs for cleanup
	const successTimeoutRef = useRef(null);
	const isMountedRef = useRef(true);

	/**
	 * Update a single field in Ollama state.
	 * @param {string} field - Field name to update
	 * @param {any} value - New value
	 */
	const updateOllama = useCallback((field, value) => {
		setOllama((prev) => ({ ...prev, [field]: value }));
	}, []);

	/**
	 * Update multiple fields in Ollama state.
	 * @param {Object} updates - Object with field updates
	 */
	const updateOllamaMultiple = useCallback((updates) => {
		setOllama((prev) => ({ ...prev, ...updates }));
	}, []);

	/**
	 * Load Ollama configuration from backend.
	 */
	const loadOllamaConfig = useCallback(async () => {
		const config = await safeInvoke("get_ollama_config", {}, null);
		if (!isMountedRef.current || !config) return;

		updateOllamaMultiple({
			url: config.url || config.default_url,
			visionModel: config.vision_model || config.default_vision_model,
			textModel: config.text_model || config.default_text_model,
			defaults: {
				url: config.default_url,
				visionModel: config.default_vision_model,
				textModel: config.default_text_model,
			},
		});
	}, [updateOllamaMultiple]);

	/**
	 * Check Ollama server status.
	 */
	/**
	 * Check Ollama server status.
	 */
	const checkOllamaStatus = useCallback(async () => {
		const status = await safeInvoke(
			"get_ollama_status",
			{},
			DEFAULT_OLLAMA_STATUS
		);
		if (isMountedRef.current) {
			updateOllama("status", status);
		}
	}, [updateOllama]);

	// Load Ollama config on mount and cleanup on unmount
	useEffect(() => {
		isMountedRef.current = true;
		loadOllamaConfig();

		return () => {
			isMountedRef.current = false;
			if (successTimeoutRef.current) {
				clearTimeout(successTimeoutRef.current);
			}
		};
	}, [loadOllamaConfig]);

	// Auto-focus input on mount
	useEffect(() => {
		inputRef.current?.focus();
	}, []);

	/**
	 * Clear the runtime/saved API key (reverting to env if available).
	 */
	const handleClearKey = useCallback(async () => {
		setIsLoading(true);
		setError("");
		try {
			await invoke("clear_api_key");
			// Check if we fell back to env key or if it's now missing
			const configured = await invoke("check_api_key");
			if (configured) {
				// If we still have a key (from env), notify success but stay
				setError("Using environment key"); // Not strictly an error, but feedback
				// We call onKeySet to refresh parent state
				onKeySet?.();
			} else {
				setApiKey("");
				setError("");
				onKeySet?.(); // Will update parent status to not configured
			}
		} catch (err) {
			console.error("Failed to clear key:", err);
			setError("Failed to clear key");
		} finally {
			setIsLoading(false);
		}
	}, [onKeySet]);

	/**
	 * Handle Gemini API key submission.
	 */
	const handleSubmit = useCallback(
		async (e) => {
			e.preventDefault();
			e.stopPropagation();

			const trimmedKey = apiKey.trim();
			if (!trimmedKey) {
				setError("Please enter an API key");
				return;
			}

			setIsLoading(true);
			setError("");

			try {
				await invoke("validate_api_key", { apiKey: trimmedKey });
				await invoke("set_api_key", { apiKey: trimmedKey });
				onKeySet?.();
			} catch (err) {
				console.error("[ApiKeyInput] Validation failed:", err);
				setError(typeof err === "string" ? err : "Invalid API key");
				inputRef.current?.focus();
				inputRef.current?.select();
			} finally {
				setIsLoading(false);
			}
		},
		[apiKey, onKeySet]
	);

	/**
	 * Save Ollama configuration.
	 */
	const handleSaveOllama = useCallback(async () => {
		// Validate URL before saving
		if (!isValidUrl(ollama.url)) {
			updateOllama(
				"error",
				"Please enter a valid URL (e.g., http://localhost:11434)"
			);
			return;
		}

		updateOllamaMultiple({ error: "", success: "", saving: true });

		// Clear any existing timeout
		if (successTimeoutRef.current) {
			clearTimeout(successTimeoutRef.current);
		}

		const result = await safeInvoke(
			"set_ollama_config",
			{
				url: ollama.url,
				visionModel: ollama.visionModel,
				textModel: ollama.textModel,
			},
			null
		);

		if (result !== null) {
			await checkOllamaStatus();
			if (!isMountedRef.current) return;

			updateOllamaMultiple({
				success: "Configuration saved successfully!",
				saving: false,
			});

			// Clear success message after 3 seconds with cleanup
			successTimeoutRef.current = setTimeout(() => {
				if (isMountedRef.current) {
					updateOllama("success", "");
				}
			}, 3000);

			onKeySet?.();
		} else {
			if (isMountedRef.current) {
				updateOllamaMultiple({
					error: "Failed to save config",
					saving: false,
				});
			}
		}
	}, [
		ollama.url,
		ollama.visionModel,
		ollama.textModel,
		onKeySet,
		checkOllamaStatus,
		updateOllama,
		updateOllamaMultiple,
	]);

	/**
	 * Reset Ollama to defaults.
	 */
	const handleResetOllama = useCallback(async () => {
		updateOllamaMultiple({ saving: true, error: "" });

		const config = await safeInvoke("reset_ollama_config", {}, null);

		if (!isMountedRef.current) return;

		if (config) {
			updateOllamaMultiple({
				url: config.url,
				visionModel: config.vision_model,
				textModel: config.text_model,
				saving: false,
			});
		} else {
			updateOllamaMultiple({
				error: "Failed to reset config",
				saving: false,
			});
		}
	}, [updateOllamaMultiple]);

	/**
	 * Handle Gemini API key input change.
	 */
	const handleChange = (e) => {
		setApiKey(e.target.value);
		setError("");
	};

	/**
	 * Handle Ollama input changes - clear errors on user input.
	 */
	const handleOllamaFieldChange = useCallback(
		(field) => (e) => {
			updateOllamaMultiple({ [field]: e.target.value, error: "" });
		},
		[updateOllamaMultiple]
	);

	/**
	 * Toggle API key visibility.
	 */
	const toggleShowKey = (e) => {
		e.preventDefault();
		e.stopPropagation();
		setShowKey((prev) => !prev);
	};

	/**
	 * Toggle Ollama section visibility and check status.
	 */
	const toggleOllama = useCallback(() => {
		setOllama((prev) => {
			// Check status when opening (not closing)
			if (!prev.expanded) {
				checkOllamaStatus();
			}
			return { ...prev, expanded: !prev.expanded };
		});
	}, [checkOllamaStatus]);

	/** Prevent event propagation for drag handling */
	const stopPropagation = (e) => e.stopPropagation();

	return (
		<div
			className="api-key-container"
			role="region"
			aria-label="AI Configuration"
		>
			{/* Gemini Section */}
			<div className="api-key-header" id="api-key-title">
				🔑 Gemini API Key
			</div>
			<p className="api-key-description" id="api-key-desc">
				Enter your Gemini API key for cloud AI (recommended).
			</p>

			{apiKeySource === "env" && (
				<div className="env-key-badge">
					<span aria-hidden="true">🔒</span> Using key from
					environment variable (.env)
				</div>
			)}

			<form onSubmit={handleSubmit} className="api-key-form">
				<div className="api-key-input-wrapper">
					<input
						ref={inputRef}
						type={showKey ? "text" : "password"}
						className={`api-key-input ${error ? "has-error" : ""}`}
						placeholder={
							apiKeySource === "user"
								? "Enter new key to override..."
								: apiKeySource === "env"
									? "Enter key to override env..."
									: "Enter Gemini API key..."
						}
						value={apiKey}
						onChange={handleChange}
						disabled={isLoading}
						autoComplete="off"
						spellCheck="false"
						aria-label="Gemini API key"
						aria-invalid={!!error}
						aria-describedby={
							error ? "gemini-error" : "api-key-desc"
						}
						onMouseDown={stopPropagation}
					/>
					<button
						type="button"
						className="api-key-toggle"
						onClick={toggleShowKey}
						onMouseDown={stopPropagation}
						aria-label={showKey ? "Hide API key" : "Show API key"}
						tabIndex={-1}
					>
						{showKey ? "👁️" : "👁️‍🗨️"}
					</button>
				</div>
				<div className="api-key-actions">
					<button
						type="submit"
						className="api-key-submit"
						disabled={isLoading || !apiKey.trim()}
						onMouseDown={stopPropagation}
					>
						{isLoading ? (
							<>
								<span
									className="loading-spinner"
									aria-hidden="true"
								/>
								Validating...
							</>
						) : (
							"Save Key"
						)}
					</button>
					{apiKeySource === "user" && (
						<button
							type="button"
							className="api-key-clear"
							onClick={handleClearKey}
							disabled={isLoading}
							onMouseDown={stopPropagation}
							title="Clear saved key (revert to env)"
						>
							Clear Saved
						</button>
					)}
				</div>
			</form>
			{error && (
				<div
					id="gemini-error"
					className={`api-key-error ${error.includes("Using environment") ? "info" : ""}`}
					role="alert"
				>
					{error.includes("Using environment") ? "ℹ️" : "⚠️"} {error}
				</div>
			)}
			<a
				href="https://aistudio.google.com/apikey"
				target="_blank"
				rel="noopener noreferrer"
				className="api-key-link"
				onMouseDown={stopPropagation}
			>
				Get a free Gemini API key →
			</a>

			{/* Ollama Section Toggle */}
			<button
				type="button"
				className="ollama-toggle"
				onClick={toggleOllama}
				onMouseDown={stopPropagation}
				aria-expanded={ollama.expanded}
				aria-controls="ollama-section"
			>
				{ollama.expanded ? "▼" : "▶"} Local AI (Ollama) - Free
				Alternative
			</button>

			{ollama.expanded && (
				<div id="ollama-section" className="ollama-section">
					<p className="ollama-description">
						Run AI locally without API costs. Requires{" "}
						<a
							href="https://ollama.com/download"
							target="_blank"
							rel="noopener noreferrer"
							onMouseDown={stopPropagation}
						>
							Ollama
						</a>{" "}
						to be installed and running.
					</p>

					{/* Status indicator */}
					{ollama.status && (
						<div
							className={`ollama-status ${ollama.status.ollama_available ? "available" : "unavailable"}`}
							role="status"
							aria-live="polite"
						>
							{ollama.status.ollama_available
								? "✅ Ollama Running"
								: "❌ Ollama Not Detected"}
							{ollama.status.active_provider && (
								<span className="active-provider">
									Active: {ollama.status.active_provider}
								</span>
							)}
						</div>
					)}

					<div className="ollama-field">
						<label htmlFor="ollama-url">Server URL</label>
						<input
							id="ollama-url"
							type="url"
							value={ollama.url}
							onChange={handleOllamaFieldChange("url")}
							placeholder={ollama.defaults.url}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-url-hint"
						/>
						<span id="ollama-url-hint" className="default-hint">
							Default: {ollama.defaults.url}
						</span>
					</div>

					<div className="ollama-field">
						<label htmlFor="ollama-vision-model">
							Vision Model (for images)
						</label>
						<input
							id="ollama-vision-model"
							type="text"
							value={ollama.visionModel}
							onChange={handleOllamaFieldChange("visionModel")}
							placeholder={ollama.defaults.visionModel}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-vision-hint"
						/>
						<span id="ollama-vision-hint" className="default-hint">
							Default: {ollama.defaults.visionModel}
						</span>
					</div>

					<div className="ollama-field">
						<label htmlFor="ollama-text-model">
							Text Model (for fast tasks)
						</label>
						<input
							id="ollama-text-model"
							type="text"
							value={ollama.textModel}
							onChange={handleOllamaFieldChange("textModel")}
							placeholder={ollama.defaults.textModel}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-text-hint"
						/>
						<span id="ollama-text-hint" className="default-hint">
							Default: {ollama.defaults.textModel}
						</span>
					</div>

					{ollama.error && (
						<div className="api-key-error" role="alert">
							⚠️ {ollama.error}
						</div>
					)}

					{ollama.success && (
						<div
							className="ollama-success"
							role="status"
							aria-live="polite"
						>
							✅ {ollama.success}
						</div>
					)}

					<div className="ollama-buttons">
						<button
							type="button"
							className="ollama-save"
							onClick={handleSaveOllama}
							disabled={ollama.saving}
							onMouseDown={stopPropagation}
						>
							{ollama.saving ? "Saving..." : "Save Ollama Config"}
						</button>
						<button
							type="button"
							className="ollama-reset"
							onClick={handleResetOllama}
							disabled={ollama.saving}
							onMouseDown={stopPropagation}
						>
							{ollama.saving
								? "Resetting..."
								: "Reset to Defaults"}
						</button>
						<button
							type="button"
							className="ollama-refresh"
							onClick={checkOllamaStatus}
							disabled={ollama.saving}
							onMouseDown={stopPropagation}
							title="Refresh Ollama status"
							aria-label="Refresh Ollama status"
						>
							🔄
						</button>
					</div>

					<div className="ollama-install-guide">
						<strong>Quick Setup:</strong>
						<ol>
							<li>
								<a
									href="https://ollama.com/download"
									target="_blank"
									rel="noopener noreferrer"
								>
									Download Ollama
								</a>
							</li>
							<li>
								Run:{" "}
								<code>
									ollama pull {ollama.defaults.visionModel}
								</code>
							</li>
							<li>
								Run:{" "}
								<code>
									ollama pull {ollama.defaults.textModel}
								</code>
							</li>
							<li>
								Start: <code>ollama serve</code>
							</li>
						</ol>
					</div>
				</div>
			)}
		</div>
	);
};

ApiKeyInput.propTypes = {
	onKeySet: PropTypes.func,
};

ApiKeyInput.defaultProps = {
	onKeySet: undefined,
};

export default React.memo(ApiKeyInput);
