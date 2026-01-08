/**
 * @fileoverview System status component showing Chrome/extension detection
 * and providing contextual action buttons.
 * @module SystemStatus
 */

import React, { useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Status levels for the indicator dot
 * @type {Object}
 */
const STATUS_LEVELS = {
	CONNECTED: "connected",
	WARNING: "warning",
	ERROR: "error",
	CHECKING: "checking",
};

/**
 * Get appropriate status level based on system state
 * @param {Object} status - System status object
 * @returns {string} Status level
 */
const getStatusLevel = (status) => {
	if (!status.chromeInstalled) return STATUS_LEVELS.ERROR;
	if (!status.extensionConnected) return STATUS_LEVELS.WARNING;
	return STATUS_LEVELS.CONNECTED;
};

/**
 * Get status message based on system state
 * @param {Object} status - System status object
 * @returns {string} Status message
 */
const getStatusMessage = (status) => {
	if (!status.chromeInstalled) return "Browser not detected";
	if (!status.extensionConnected) return "Extension not connected";
	return "Connected";
};

/**
 * SystemStatusBanner component - Non-blocking status indicator with actions
 *
 * @param {Object} props - Component props
 * @param {Object} props.status - System status from backend
 * @param {boolean} props.extensionConnected - Live extension connection state
 * @param {function} [props.onStatusChange] - Callback when status changes
 * @returns {JSX.Element} Status banner component
 */
const SystemStatusBanner = ({ status, extensionConnected, onStatusChange }) => {
	const [isExpanded, setIsExpanded] = useState(false);
	const [isLaunching, setIsLaunching] = useState(false);

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
	 * Handle Chrome installation - opens download page
	 */
	const handleGetChrome = useCallback(async () => {
		try {
			// Use the opener plugin (permitted via opener:default)
			await openUrl("https://www.google.com/chrome/");
		} catch (err) {
			console.error("Failed to open Chrome download:", err);
			// Fallback to window.open
			window.open("https://www.google.com/chrome/", "_blank");
		}
	}, []);

	/**
	 * Handle Chrome launch
	 */
	const handleLaunchChrome = useCallback(async () => {
		setIsLaunching(true);
		try {
			await invoke("launch_chrome", { url: null });
		} catch (err) {
			console.error("Failed to launch Chrome:", err);
			alert("Could not launch Chrome. Please open it manually.");
		} finally {
			setIsLaunching(false);
		}
	}, []);

	/**
	 * Handle extension installation - opens extensions page
	 * Uses unified backend command for cross-platform support
	 */
	const handleInstallExtension = useCallback(async () => {
		try {
			await invoke("launch_chrome", { url: "chrome://extensions" });
		} catch (err) {
			console.error("Failed to open extensions page:", err);
			// Show manual instructions as fallback
			alert(
				'To install the extension:\n\n1. Open Chrome and go to: chrome://extensions\n2. Enable "Developer mode"\n3. Click "Load unpacked"\n4. Select the ghost-extension folder'
			);
		}
	}, []);

	// If everything is connected, show minimal badge
	if (statusLevel === STATUS_LEVELS.CONNECTED && !isExpanded) {
		return (
			<div
				className="system-status-banner collapsed"
				onClick={() => setIsExpanded(true)}
				onKeyDown={(e) => {
					if (e.key === "Enter" || e.key === " ") {
						e.preventDefault();
						setIsExpanded(true);
					}
				}}
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

	return (
		<div
			className={`system-status-banner ${isExpanded ? "expanded" : ""}`}
			role="region"
			aria-label="System status"
		>
			{/* Header - Always visible */}
			<div
				className="status-header"
				onClick={() => setIsExpanded(!isExpanded)}
				onKeyDown={(e) => {
					if (e.key === "Enter" || e.key === " ") {
						e.preventDefault();
						setIsExpanded(!isExpanded);
					}
				}}
				role="button"
				tabIndex={0}
				aria-expanded={isExpanded}
			>
				<div className="status-indicator">
					<span
						className={`status-dot ${statusLevel}`}
						aria-hidden="true"
					/>
					<span className="status-text">{statusMessage}</span>
				</div>
				<span className="expand-icon" aria-hidden="true">
					{isExpanded ? "▼" : "▶"}
				</span>
			</div>

			{/* Expandable content */}
			{(isExpanded || statusLevel !== STATUS_LEVELS.CONNECTED) && (
				<div className="status-content">
					{/* Chrome Status */}
					<div className="status-row">
						<span className="status-label">
							{effectiveStatus.chromeInstalled ? "🌐" : "⚠️"}{" "}
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
							{effectiveStatus.extensionConnected ? "🔌" : "⚠️"}{" "}
							Extension
						</span>
						<span className="status-value">
							{effectiveStatus.extensionConnected
								? "Connected"
								: "Not Connected"}
						</span>
					</div>

					{/* Action Buttons */}
					<div className="status-actions">
						{!effectiveStatus.chromeInstalled && (
							<button
								className="status-action-btn primary"
								onClick={(e) => {
									e.stopPropagation();
									handleGetChrome();
								}}
								onMouseDown={(e) => e.stopPropagation()}
							>
								📥 Get Chrome
							</button>
						)}

						{effectiveStatus.chromeInstalled &&
							!effectiveStatus.extensionConnected && (
								<>
									<button
										className="status-action-btn"
										onClick={(e) => {
											e.stopPropagation();
											handleLaunchChrome();
										}}
										onMouseDown={(e) => e.stopPropagation()}
										disabled={isLaunching}
									>
										{isLaunching
											? "⏳ Launching..."
											: "🚀 Launch Chrome"}
									</button>
									<button
										className="status-action-btn primary"
										onClick={(e) => {
											e.stopPropagation();
											handleInstallExtension();
										}}
										onMouseDown={(e) => e.stopPropagation()}
									>
										📦 Install Extension
									</button>
								</>
							)}
					</div>

					{/* Fallback Mode Notice */}
					{!effectiveStatus.extensionConnected && (
						<div className="fallback-notice">
							<span className="fallback-icon">📸</span>
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

export default SystemStatusBanner;
