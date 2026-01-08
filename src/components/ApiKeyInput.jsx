/**
 * @fileoverview API Key and AI Provider configuration component.
 * Supports both Gemini API (cloud) and Ollama (local) AI providers.
 * @module ApiKeyInput
 */

import React, { useState, useCallback, useRef, useEffect } from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";

/** Default Ollama status when unavailable */
const DEFAULT_OLLAMA_STATUS = {
	ollama_available: false,
	active_provider: "None",
};

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
 * @returns {JSX.Element} AI configuration form
 */
const ApiKeyInput = ({ onKeySet }) => {
	// Gemini state
	const [apiKey, setApiKey] = useState("");
	const [error, setError] = useState("");
	const [isLoading, setIsLoading] = useState(false);
	const [showKey, setShowKey] = useState(false);
	const inputRef = useRef(null);

	// Ollama state
	const [showOllama, setShowOllama] = useState(false);
	const [ollamaUrl, setOllamaUrl] = useState("");
	const [ollamaVisionModel, setOllamaVisionModel] = useState("");
	const [ollamaTextModel, setOllamaTextModel] = useState("");
	const [ollamaDefaults, setOllamaDefaults] = useState({});
	const [ollamaStatus, setOllamaStatus] = useState(null);
	const [ollamaError, setOllamaError] = useState("");
	const [ollamaSuccess, setOllamaSuccess] = useState("");
	const [ollamaSaving, setOllamaSaving] = useState(false);

	// Refs for cleanup
	const successTimeoutRef = useRef(null);
	const isMountedRef = useRef(true);

	/**
	 * Load Ollama configuration from backend.
	 */
	const loadOllamaConfig = useCallback(async () => {
		try {
			const config = await invoke("get_ollama_config");
			if (!isMountedRef.current) return;

			setOllamaUrl(config.url || config.default_url);
			setOllamaVisionModel(
				config.vision_model || config.default_vision_model
			);
			setOllamaTextModel(config.text_model || config.default_text_model);
			setOllamaDefaults({
				url: config.default_url,
				visionModel: config.default_vision_model,
				textModel: config.default_text_model,
			});
		} catch (err) {
			console.error("[ApiKeyInput] Failed to load Ollama config:", err);
		}
	}, []);

	/**
	 * Check Ollama server status.
	 */
	const checkOllamaStatus = useCallback(async () => {
		try {
			const status = await invoke("get_ollama_status");
			if (isMountedRef.current) {
				setOllamaStatus(status);
			}
		} catch {
			if (isMountedRef.current) {
				setOllamaStatus(DEFAULT_OLLAMA_STATUS);
			}
		}
	}, []);

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
		if (!isValidUrl(ollamaUrl)) {
			setOllamaError(
				"Please enter a valid URL (e.g., http://localhost:11434)"
			);
			return;
		}

		setOllamaError("");
		setOllamaSuccess("");
		setOllamaSaving(true);

		// Clear any existing timeout
		if (successTimeoutRef.current) {
			clearTimeout(successTimeoutRef.current);
		}

		try {
			await invoke("set_ollama_config", {
				url: ollamaUrl,
				visionModel: ollamaVisionModel,
				textModel: ollamaTextModel,
			});
			await checkOllamaStatus();

			if (!isMountedRef.current) return;

			setOllamaSuccess("Configuration saved successfully!");

			// Clear success message after 3 seconds with cleanup
			successTimeoutRef.current = setTimeout(() => {
				if (isMountedRef.current) {
					setOllamaSuccess("");
				}
			}, 3000);

			onKeySet?.();
		} catch (err) {
			if (isMountedRef.current) {
				setOllamaError(
					typeof err === "string" ? err : "Failed to save config"
				);
			}
		} finally {
			if (isMountedRef.current) {
				setOllamaSaving(false);
			}
		}
	}, [
		ollamaUrl,
		ollamaVisionModel,
		ollamaTextModel,
		onKeySet,
		checkOllamaStatus,
	]);

	/**
	 * Reset Ollama to defaults.
	 */
	const handleResetOllama = useCallback(async () => {
		setOllamaSaving(true);
		setOllamaError("");

		try {
			const config = await invoke("reset_ollama_config");
			if (!isMountedRef.current) return;

			setOllamaUrl(config.url);
			setOllamaVisionModel(config.vision_model);
			setOllamaTextModel(config.text_model);
		} catch {
			if (isMountedRef.current) {
				setOllamaError("Failed to reset config");
			}
		} finally {
			if (isMountedRef.current) {
				setOllamaSaving(false);
			}
		}
	}, []);

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
	const handleOllamaUrlChange = (e) => {
		setOllamaUrl(e.target.value);
		setOllamaError("");
	};

	const handleOllamaVisionModelChange = (e) => {
		setOllamaVisionModel(e.target.value);
		setOllamaError("");
	};

	const handleOllamaTextModelChange = (e) => {
		setOllamaTextModel(e.target.value);
		setOllamaError("");
	};

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
		setShowOllama((prev) => {
			// Check status when opening (not closing)
			if (!prev) {
				checkOllamaStatus();
			}
			return !prev;
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
			<form onSubmit={handleSubmit} className="api-key-form">
				<div className="api-key-input-wrapper">
					<input
						ref={inputRef}
						type={showKey ? "text" : "password"}
						className={`api-key-input ${error ? "has-error" : ""}`}
						placeholder="Enter Gemini API key..."
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
			</form>
			{error && (
				<div id="gemini-error" className="api-key-error" role="alert">
					⚠️ {error}
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
				aria-expanded={showOllama}
				aria-controls="ollama-section"
			>
				{showOllama ? "▼" : "▶"} Local AI (Ollama) - Free Alternative
			</button>

			{showOllama && (
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
					{ollamaStatus && (
						<div
							className={`ollama-status ${ollamaStatus.ollama_available ? "available" : "unavailable"}`}
							role="status"
							aria-live="polite"
						>
							{ollamaStatus.ollama_available
								? "✅ Ollama Running"
								: "❌ Ollama Not Detected"}
							{ollamaStatus.active_provider && (
								<span className="active-provider">
									Active: {ollamaStatus.active_provider}
								</span>
							)}
						</div>
					)}

					<div className="ollama-field">
						<label htmlFor="ollama-url">Server URL</label>
						<input
							id="ollama-url"
							type="url"
							value={ollamaUrl}
							onChange={handleOllamaUrlChange}
							placeholder={ollamaDefaults.url}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-url-hint"
						/>
						<span id="ollama-url-hint" className="default-hint">
							Default: {ollamaDefaults.url}
						</span>
					</div>

					<div className="ollama-field">
						<label htmlFor="ollama-vision-model">
							Vision Model (for images)
						</label>
						<input
							id="ollama-vision-model"
							type="text"
							value={ollamaVisionModel}
							onChange={handleOllamaVisionModelChange}
							placeholder={ollamaDefaults.visionModel}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-vision-hint"
						/>
						<span id="ollama-vision-hint" className="default-hint">
							Default: {ollamaDefaults.visionModel}
						</span>
					</div>

					<div className="ollama-field">
						<label htmlFor="ollama-text-model">
							Text Model (for fast tasks)
						</label>
						<input
							id="ollama-text-model"
							type="text"
							value={ollamaTextModel}
							onChange={handleOllamaTextModelChange}
							placeholder={ollamaDefaults.textModel}
							onMouseDown={stopPropagation}
							aria-describedby="ollama-text-hint"
						/>
						<span id="ollama-text-hint" className="default-hint">
							Default: {ollamaDefaults.textModel}
						</span>
					</div>

					{ollamaError && (
						<div className="api-key-error" role="alert">
							⚠️ {ollamaError}
						</div>
					)}

					{ollamaSuccess && (
						<div
							className="ollama-success"
							role="status"
							aria-live="polite"
						>
							✅ {ollamaSuccess}
						</div>
					)}

					<div className="ollama-buttons">
						<button
							type="button"
							className="ollama-save"
							onClick={handleSaveOllama}
							disabled={ollamaSaving}
							onMouseDown={stopPropagation}
						>
							{ollamaSaving ? "Saving..." : "Save Ollama Config"}
						</button>
						<button
							type="button"
							className="ollama-reset"
							onClick={handleResetOllama}
							disabled={ollamaSaving}
							onMouseDown={stopPropagation}
						>
							{ollamaSaving
								? "Resetting..."
								: "Reset to Defaults"}
						</button>
						<button
							type="button"
							className="ollama-refresh"
							onClick={checkOllamaStatus}
							disabled={ollamaSaving}
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
									ollama pull {ollamaDefaults.visionModel}
								</code>
							</li>
							<li>
								Run:{" "}
								<code>
									ollama pull {ollamaDefaults.textModel}
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
