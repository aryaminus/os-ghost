/**
 * @fileoverview System status component showing Chrome/extension detection
 * and providing contextual action buttons.
 * @module SystemStatus
 */

import React, {
	useState,
	useCallback,
	useMemo,
	useRef,
	useEffect,
} from "react";
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../utils/data";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Status levels for the indicator dot
 * @readonly
 * @enum {string}
 */
const STATUS_LEVELS = Object.freeze({
	CONNECTED: "connected",
	WARNING: "warning",
	ERROR: "error",
	CHECKING: "checking",
});

/** Auto-dismiss timeout for error messages (ms) */
const ERROR_DISMISS_TIMEOUT = 5000;

/**
 * Extension installation instructions for manual fallback.
 */
const EXTENSION_INSTALL_INSTRUCTIONS = [
	"1. Click 'Install Extension' to open the Chrome Web Store",
	'2. Click "Add to Chrome"',
	'3. Confirm "Add Extension"',
].join("\n");

/**
 * Get appropriate status level based on system state.
 * @param {Object} status - System status object
 * @param {boolean} status.chromeInstalled - Whether Chrome is installed
 * @param {boolean} status.extensionConnected - Whether extension is connected
 * @returns {string} Status level
 */
const getStatusLevel = (status) => {
	if (!status.chromeInstalled) return STATUS_LEVELS.ERROR;
	if (!status.extensionConnected) return STATUS_LEVELS.WARNING;
	return STATUS_LEVELS.CONNECTED;
};

/**
 * Get status message based on system state.
 * @param {Object} status - System status object
 * @returns {string} Status message
 */
const getStatusMessage = (status) => {
	if (!status.chromeInstalled) return "Browser not detected";
	if (!status.extensionConnected) return "Extension not connected";
	return "Connected";
};

/**
 * SystemStatusBanner component - Non-blocking status indicator with actions.
 * Supports controlled accordion state for persistence across parent re-renders.
 *
 * @param {Object} props - Component props
 * @param {Object} props.status - System status from backend (Tauri IPC)
 * @param {boolean} props.status.chromeInstalled - Whether Chrome is installed
 * @param {boolean} props.status.extensionConnected - Whether extension is connected
 * @param {boolean} [props.extensionConnected=false] - Live extension connection state (WebSocket)
 * @param {boolean} [props.isExpanded=false] - Controlled accordion expanded state
 * @param {function} [props.onToggleExpand] - Callback to toggle expanded state (receives boolean or updater function)
 * @param {boolean} [props.flat=false] - If true, removes borders/accordion behavior and shows full content
 * @returns {JSX.Element} Status banner component
 */
const SystemStatusBanner = ({
	status,
	extensionConnected,
	isExpanded = false,
	onToggleExpand,
	flat = false,
}) => {
	const [isLaunching, setIsLaunching] = useState(false);
	const [actionError, setActionError] = useState(null);

	// Refs for cleanup
	const isMountedRef = useRef(true);
	const errorTimeoutRef = useRef(null);

	// Cleanup on unmount
	useEffect(() => {
		isMountedRef.current = true;
		return () => {
			isMountedRef.current = false;
			if (errorTimeoutRef.current) {
				clearTimeout(errorTimeoutRef.current);
			}
		};
	}, []);

	/**
	 * Display an error message that auto-dismisses.
	 * @param {string} message - Error message to display
	 */
	const showError = useCallback((message) => {
		// Clear any existing timeout
		if (errorTimeoutRef.current) {
			clearTimeout(errorTimeoutRef.current);
		}

		setActionError(message);

		// Auto-dismiss after timeout
		errorTimeoutRef.current = setTimeout(() => {
			if (isMountedRef.current) {
				setActionError(null);
			}
		}, ERROR_DISMISS_TIMEOUT);
	}, []);

	/**
	 * Clear the current error message.
	 */
	const clearError = useCallback(() => {
		if (errorTimeoutRef.current) {
			clearTimeout(errorTimeoutRef.current);
		}
		setActionError(null);
	}, []);

	// Combine backend status with live extension state
	const effectiveStatus = useMemo(
		() => ({
			...status,
			extensionConnected: extensionConnected || status.extensionConnected,
		}),
		[status, extensionConnected]
	);

	const statusLevel = getStatusLevel(effectiveStatus);
	const statusMessage = getStatusMessage(effectiveStatus);

	/**
	 * Handle Chrome installation - opens download page.
	 */
	const handleGetChrome = async () => {
		clearError();
		try {
			// Use the opener plugin (permitted via opener:default)
			await openUrl("https://www.google.com/chrome/");
		} catch (err) {
			console.error("Failed to open Chrome download:", err);
			// Fallback to window.open
			window.open("https://www.google.com/chrome/", "_blank");
		}
	};

	/**
	 * Handle Chrome launch.
	 */
	const handleLaunchChrome = async () => {
		clearError();
		setIsLaunching(true);
		try {
			await invoke("launch_chrome", { url: null });
		} catch (err) {
			console.error("Failed to launch Chrome:", err);
			if (isMountedRef.current) {
				showError("Could not launch Chrome. Please open it manually.");
			}
		} finally {
			if (isMountedRef.current) {
				setIsLaunching(false);
			}
		}
	};

	/**
	 * Handle extension installation - opens Web Store page.
	 * Uses unified backend command for cross-platform support.
	 */
	const handleInstallExtension = async () => {
		clearError();
		const WEB_STORE_URL =
			"https://chromewebstore.google.com/detail/os-ghost-bridge/iakaaklohlcdhoalipmmljopmjnhbcdn";
		// Try to open via Tauri's launcher first to open in default browser
		try {
			await invoke("launch_chrome", { url: WEB_STORE_URL });
		} catch (err) {
			console.error("Failed to open Web Store page:", err);
			if (isMountedRef.current) {
				showError(
					`To install the extension:\n${EXTENSION_INSTALL_INSTRUCTIONS}`
				);
			}
		}
	};

	const toggleExpanded = useCallback(() => {
		if (flat) return;
		onToggleExpand?.((prev) => !prev);
	}, [onToggleExpand, flat]);

	const expand = useCallback(() => {
		if (flat) return;
		onToggleExpand?.(true);
	}, [onToggleExpand, flat]);

	/**
	 * Handle keyboard interaction for toggle.
	 * @param {React.KeyboardEvent} e - Keyboard event
	 * @param {function} action - Action to perform
	 */
	const handleKeyDown = (e, action) => {
		if (flat) return;
		if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			action();
		}
	};

	/** Prevent event propagation for drag handling */
	const stopPropagation = (e) => e.stopPropagation();

	/**
	 * Wrap a handler to stop propagation.
	 * @param {function} handler - Handler function
	 * @returns {function} Wrapped handler
	 */
	const withStopPropagation = (handler) => (e) => {
		e.stopPropagation();
		handler();
	};

	// If flat mode, force expanded behavior and simplified rendering
	const effectiveExpanded = flat || isExpanded;

	// If everything is connected, show minimal badge (ONLY if not flat)
	if (!flat && statusLevel === STATUS_LEVELS.CONNECTED && !isExpanded) {
		return (
			<div
				className="system-status-banner collapsed"
				onClick={expand}
				onKeyDown={(e) => handleKeyDown(e, expand)}
				role="button"
				tabIndex={0}
				aria-label="System status: Connected. Click to expand."
			>
				<div className="status-indicator">
					<span
						className={`status-dot ${statusLevel}`}
						aria-hidden="true"
					/>
					<span className="status-text-mini">🔗 Connected</span>
				</div>
			</div>
		);
	}

	const contentId = "system-status-content";

	return (
		<div
			className={`system-status-banner ${effectiveExpanded ? "expanded" : ""} ${flat ? "flat" : ""}`}
			role="region"
			aria-label="System status"
		>
			{/* Header - Always visible */}
			<div
				className="status-header"
				onClick={toggleExpanded}
				onKeyDown={(e) => handleKeyDown(e, toggleExpanded)}
				role={flat ? undefined : "button"}
				tabIndex={flat ? -1 : 0}
				aria-expanded={flat ? undefined : effectiveExpanded}
				aria-controls={contentId}
				style={{ cursor: flat ? "default" : "pointer" }}
			>
				<div className="status-indicator">
					<span
						className={`status-dot ${statusLevel}`}
						aria-hidden="true"
					/>
					<span className="status-text">{statusMessage}</span>
				</div>
				{!flat && (
					<span className="expand-icon" aria-hidden="true">
						{effectiveExpanded ? "▼" : "▶"}
					</span>
				)}
			</div>

			{/* Expandable content */}
			{effectiveExpanded && (
				<div id={contentId} className="status-content">
					{/* Error Banner */}
					{actionError && (
						<div
							className="status-error-banner"
							role="alert"
							aria-live="assertive"
						>
							<span className="error-icon" aria-hidden="true">
								⚠️
							</span>
							<span className="error-message">{actionError}</span>
							<button
								type="button"
								className="error-dismiss"
								onClick={clearError}
								aria-label="Dismiss error"
							>
								×
							</button>
						</div>
					)}

					{/* Chrome Status */}
					<div className="status-row">
						<span className="status-label">
							<span aria-hidden="true">
								{effectiveStatus.chromeInstalled ? "🌐" : "⚠️"}
							</span>{" "}
							Browser
						</span>
						<span className="status-value">
							{effectiveStatus.chromeInstalled
								? "Installed"
								: "Not Found"}
						</span>
					</div>

					{/* Extension Status */}
					<div className="status-row">
						<span className="status-label">
							<span aria-hidden="true">
								{effectiveStatus.extensionConnected
									? "🔌"
									: "⚠️"}
							</span>{" "}
							Extension
						</span>
						<span className="status-value">
							{effectiveStatus.extensionConnected
								? "Connected"
								: "Not Connected"}
						</span>
					</div>

					{/* Action Buttons */}
					<div
						className="status-actions"
						role="group"
						aria-label="Actions"
					>
						{!effectiveStatus.chromeInstalled && (
							<button
								type="button"
								className="status-action-btn primary"
								onClick={withStopPropagation(handleGetChrome)}
								onMouseDown={stopPropagation}
							>
								📥 Get Chrome
							</button>
						)}

						{effectiveStatus.chromeInstalled &&
							!effectiveStatus.extensionConnected && (
								<>
									<button
										type="button"
										className="status-action-btn"
										onClick={withStopPropagation(
											handleLaunchChrome
										)}
										onMouseDown={stopPropagation}
										disabled={isLaunching}
										aria-busy={isLaunching}
									>
										{isLaunching
											? "⏳ Launching..."
											: "🚀 Launch Chrome"}
									</button>
									<button
										type="button"
										className="status-action-btn primary"
										onClick={withStopPropagation(
											handleInstallExtension
										)}
										onMouseDown={stopPropagation}
									>
										📦 Install Extension
									</button>
								</>
							)}
					</div>

					{/* Fallback Mode Notice */}
					{!effectiveStatus.extensionConnected && (
						<div className="fallback-notice" role="note">
							<span className="fallback-icon" aria-hidden="true">
								📸
							</span>
							<span className="fallback-text">
								Running in screenshot mode. Install extension
								for real-time browser tracking.
							</span>
						</div>
					)}
				</div>
			)}
		</div>
	);
};

SystemStatusBanner.propTypes = {
	status: PropTypes.shape({
		chromeInstalled: PropTypes.bool,
		extensionConnected: PropTypes.bool,
	}).isRequired,
	extensionConnected: PropTypes.bool,
	isExpanded: PropTypes.bool,
	onToggleExpand: PropTypes.func,
	flat: PropTypes.bool,
};

SystemStatusBanner.defaultProps = {
	extensionConnected: false,
	isExpanded: false,
	onToggleExpand: undefined,
	flat: false,
};

export default React.memo(SystemStatusBanner);
