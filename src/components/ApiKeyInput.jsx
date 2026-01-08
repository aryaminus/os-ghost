/**
 * @fileoverview API Key input component for entering Gemini API key.
 * Shows input form when key not configured, validates against API.
 * @module ApiKeyInput
 */

import React, { useState, useCallback, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * API Key input component.
 * Displays an input field and validates the key against Gemini API.
 *
 * @param {Object} props - Component props
 * @param {function} props.onKeySet - Callback when key is successfully set
 * @returns {JSX.Element} API key input form
 */
const ApiKeyInput = ({ onKeySet }) => {
	const [apiKey, setApiKey] = useState("");
	const [error, setError] = useState("");
	const [isLoading, setIsLoading] = useState(false);
	const [showKey, setShowKey] = useState(false);
	const inputRef = useRef(null);

	// Auto-focus input on mount
	useEffect(() => {
		if (inputRef.current) {
			inputRef.current.focus();
		}
	}, []);

	/**
	 * Handle form submission - validate and save the API key.
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
				// First validate the key
				await invoke("validate_api_key", { apiKey: trimmedKey });

				// If validation succeeds, save it
				await invoke("set_api_key", { apiKey: trimmedKey });

				// Notify parent component
				if (onKeySet) {
					onKeySet();
				}
			} catch (err) {
				console.error("[ApiKeyInput] Validation failed:", err);
				setError(typeof err === "string" ? err : "Invalid API key");
				// Focus input on error for quick retry
				if (inputRef.current) {
					inputRef.current.focus();
					inputRef.current.select();
				}
			} finally {
				setIsLoading(false);
			}
		},
		[apiKey, onKeySet]
	);

	/**
	 * Handle input change.
	 */
	const handleChange = useCallback((e) => {
		setApiKey(e.target.value);
		setError(""); // Clear error on input
	}, []);

	/**
	 * Toggle password visibility.
	 */
	const toggleShowKey = useCallback((e) => {
		e.preventDefault();
		e.stopPropagation();
		setShowKey((prev) => !prev);
	}, []);

	return (
		<div
			className="api-key-container"
			role="region"
			aria-label="API Key Setup"
		>
			<div className="api-key-header" id="api-key-title">
				🔑 API Key Required
			</div>
			<p className="api-key-description" id="api-key-desc">
				Enter your Gemini API key to enable AI features.
			</p>
			<form
				onSubmit={handleSubmit}
				className="api-key-form"
				aria-labelledby="api-key-title"
				aria-describedby="api-key-desc"
			>
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
						aria-describedby={error ? "api-key-error" : undefined}
						onMouseDown={(e) => e.stopPropagation()}
					/>
					<button
						type="button"
						className="api-key-toggle"
						onClick={toggleShowKey}
						onMouseDown={(e) => e.stopPropagation()}
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
					onMouseDown={(e) => e.stopPropagation()}
					aria-busy={isLoading}
				>
					{isLoading ? (
						<>
							<span
								className="loading-spinner"
								aria-hidden="true"
							></span>
							Validating...
						</>
					) : (
						"Save Key"
					)}
				</button>
			</form>
			{error && (
				<div
					className="api-key-error"
					id="api-key-error"
					role="alert"
					aria-live="polite"
				>
					⚠️ {error}
				</div>
			)}
			<a
				href="https://aistudio.google.com/apikey"
				target="_blank"
				rel="noopener noreferrer"
				className="api-key-link"
				onMouseDown={(e) => e.stopPropagation()}
			>
				Get a free API key →
			</a>
		</div>
	);
};

export default React.memo(ApiKeyInput);
