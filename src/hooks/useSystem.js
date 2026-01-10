/**
 * @fileoverview System status and configuration hook.
 * Handles Chrome detection, API key management, and extension connection status.
 * @module useSystem
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback, useRef } from "react";

/** Debug mode - only log in development */
const DEBUG_MODE = import.meta.env.DEV;

function log(...args) {
	if (DEBUG_MODE) console.log("[System]", ...args);
}

/**
 * System status object
 * @typedef {Object} SystemStatus
 * @property {boolean|null} chromeInstalled - null = checking, true/false = detected
 * @property {string|null} chromePath - Path to Chrome executable
 * @property {boolean} extensionConnected - Whether extension is connected via TCP
 * @property {boolean} extensionOperational - Whether extension is responding
 * @property {boolean} apiKeyConfigured - Whether Gemini API key is set
 * @property {string|null} lastKnownUrl - Last URL from extension
 * @property {string} currentMode - Current app mode (game, etc.)
 */

/**
 * Ollama configuration
 * @typedef {Object} OllamaConfig
 * @property {string} url - Ollama server URL
 * @property {string} visionModel - Vision model name
 * @property {string} textModel - Text model name
 * @property {boolean} available - Whether Ollama is running
 */

/** @type {SystemStatus} */
const initialSystemStatus = {
	chromeInstalled: null,
	chromePath: null,
	extensionConnected: false,
	extensionOperational: false,
	apiKeyConfigured: false,
	lastKnownUrl: null,
	currentMode: "game",
};

/**
 * Hook for managing system status and configuration.
 * Handles Chrome detection, API keys, Ollama config, and extension connection.
 *
 * @returns {Object} System status and control functions
 */
export function useSystem() {
	const [systemStatus, setSystemStatus] = useState(initialSystemStatus);
	const [ollamaStatus, setOllamaStatus] = useState({
		available: false,
		geminiConfigured: false,
		activeProvider: "None",
	});
	const [isCheckingStatus, setIsCheckingStatus] = useState(false);
	const isMountedRef = useRef(true);

	/**
	 * Detect system status (Chrome, extension, API key)
	 */
	const detectSystemStatus = useCallback(async () => {
		setIsCheckingStatus(true);
		try {
			const status = await invoke("detect_chrome");
			if (!isMountedRef.current) return;

			setSystemStatus((prev) => ({
				...prev,
				chromeInstalled: status.chrome_installed,
				chromePath: status.chrome_path,
				apiKeyConfigured: status.api_key_configured,
				currentMode: status.current_mode || "game",
			}));
		} catch (err) {
			console.error("[System] Detection failed:", err);
		} finally {
			if (isMountedRef.current) {
				setIsCheckingStatus(false);
			}
		}
	}, []);

	/**
	 * Check Ollama status
	 */
	const checkOllamaStatus = useCallback(async () => {
		try {
			const status = await invoke("get_ollama_status");
			if (!isMountedRef.current) return;

			setOllamaStatus({
				available: status.ollama_available,
				geminiConfigured: status.gemini_configured,
				activeProvider: status.active_provider,
			});
		} catch (err) {
			console.error("[System] Ollama status check failed:", err);
		}
	}, []);

	/**
	 * Set API key
	 * @param {string} apiKey - Gemini API key
	 */
	const setApiKey = useCallback(async (apiKey) => {
		try {
			await invoke("set_api_key", { apiKey });
			setSystemStatus((prev) => ({
				...prev,
				apiKeyConfigured: true,
			}));
			log("API key set successfully");
			return { success: true };
		} catch (err) {
			console.error("[System] Failed to set API key:", err);
			return { success: false, error: err };
		}
	}, []);

	/**
	 * Validate API key
	 * @param {string} apiKey - API key to validate
	 */
	const validateApiKey = useCallback(async (apiKey) => {
		try {
			const valid = await invoke("validate_api_key", { apiKey });
			return { valid, error: null };
		} catch (err) {
			return { valid: false, error: err };
		}
	}, []);

	/**
	 * Get Ollama configuration
	 */
	const getOllamaConfig = useCallback(async () => {
		try {
			return await invoke("get_ollama_config");
		} catch (err) {
			console.error("[System] Failed to get Ollama config:", err);
			return null;
		}
	}, []);

	/**
	 * Set Ollama configuration
	 */
	const setOllamaConfig = useCallback(async (url, visionModel, textModel) => {
		try {
			await invoke("set_ollama_config", { url, visionModel, textModel });
			await checkOllamaStatus();
			return { success: true };
		} catch (err) {
			return { success: false, error: err };
		}
	}, [checkOllamaStatus]);

	/**
	 * Reset Ollama to defaults
	 */
	const resetOllamaConfig = useCallback(async () => {
		try {
			const config = await invoke("reset_ollama_config");
			await checkOllamaStatus();
			return config;
		} catch (err) {
			console.error("[System] Failed to reset Ollama config:", err);
			return null;
		}
	}, [checkOllamaStatus]);

	/**
	 * Launch Chrome browser
	 * @param {string|null} url - Optional URL to open
	 */
	const launchChrome = useCallback(async (url = null) => {
		try {
			await invoke("launch_chrome", { url });
			return { success: true };
		} catch (err) {
			return { success: false, error: err };
		}
	}, []);

	// Set up event listeners for extension connection
	useEffect(() => {
		isMountedRef.current = true;
		const unlistenFns = [];

		const setupListeners = async () => {
			const register = async (event, callback) => {
				const unlisten = await listen(event, callback);
				unlistenFns.push(unlisten);
			};

			await register("extension_connected", () => {
				log("Extension connected");
				setSystemStatus((prev) => ({
					...prev,
					extensionConnected: true,
					extensionOperational: true,
				}));
			});

			await register("extension_disconnected", () => {
				log("Extension disconnected");
				setSystemStatus((prev) => ({
					...prev,
					extensionConnected: false,
					extensionOperational: false,
				}));
			});

			await register("system_status_update", (event) => {
				const status = event.payload;
				setSystemStatus((prev) => ({
					...prev,
					chromeInstalled: status.chrome_installed,
					chromePath: status.chrome_path,
					apiKeyConfigured: status.api_key_configured,
					currentMode: status.current_mode || "game",
				}));
			});
		};

		setupListeners();
		detectSystemStatus();
		checkOllamaStatus();

		return () => {
			isMountedRef.current = false;
			unlistenFns.forEach((fn) => fn());
		};
	}, [detectSystemStatus, checkOllamaStatus]);

	return {
		systemStatus,
		ollamaStatus,
		isCheckingStatus,
		// Actions
		detectSystemStatus,
		checkOllamaStatus,
		setApiKey,
		validateApiKey,
		getOllamaConfig,
		setOllamaConfig,
		resetOllamaConfig,
		launchChrome,
	};
}
