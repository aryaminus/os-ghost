/**
 * @fileoverview Ghost character component with ASCII art sprite, animations,
 * proximity indicator, dialogue box, and game UI.
 * @module Ghost
 */

import React, {
	useState,
	useEffect,
	useRef,
	useCallback,
	useMemo,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { Command } from "@tauri-apps/plugin-shell";
import { useGhostGame } from "../hooks/useTauriCommands";
import ApiKeyInput from "./ApiKeyInput";

/**
 * ASCII Art sprites for Ghost in different states.
 * @type {Record<string, string>}
 */
const GHOST_SPRITES = {
	idle: `
    .-.
   (o.o)
    |=|
   __|__
  //.=|=.\\\\
 // .=|=. \\\\
 \\\\ .=|=. //
  \\\\(_=_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,

	thinking: `
    .-.
   (?.?)
    |~|
   __|__
  //.~|~.\\\\
 // .~|~. \\\\
 \\\\ .~|~. //
  \\\\(_~_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,

	searching: `
    .-.
   (>.<)
    |*|
   __|__
  //*=|=*\\\\
 // *=|=* \\\\
 \\\\ *=|=* //
  \\\\(_*_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,

	celebrate: `
    \\o/
   (^.^)
    |!|
   __|__
  //!=|=!\\\\
 // !=|=! \\\\
 \\\\ !=|=! //
  \\\\(_!_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
};

/**
 * Typewriter text effect component.
 * Displays text character by character with a blinking cursor.
 *
 * @param {Object} props - Component props
 * @param {string} props.text - Text to display with typewriter effect
 * @param {number} [props.speed=30] - Milliseconds between characters
 * @returns {JSX.Element} Typewriter text element
 */
const TypewriterText = React.memo(({ text, speed = 30 }) => {
	const [displayed, setDisplayed] = useState("");
	/** @type {React.MutableRefObject<number>} */
	const indexRef = useRef(0);

	useEffect(() => {
		setDisplayed("");
		indexRef.current = 0;

		if (!text) return;

		const timer = setInterval(() => {
			if (indexRef.current < text.length) {
				setDisplayed(text.slice(0, indexRef.current + 1));
				indexRef.current++;
			} else {
				clearInterval(timer);
			}
		}, speed);

		return () => clearInterval(timer);
	}, [text, speed]);

	return (
		<span className="typewriter-text" aria-live="polite">
			{displayed}
			<span className="cursor" aria-hidden="true">
				▌
			</span>
		</span>
	);
});

TypewriterText.displayName = "TypewriterText";

/**
 * Proximity indicator bar showing hot/cold feedback.
 * Changes color based on proximity value.
 *
 * @param {Object} props - Component props
 * @param {number} props.proximity - Proximity value (0.0 - 1.0)
 * @returns {JSX.Element} Proximity bar element
 */
const ProximityBar = React.memo(({ proximity }) => {
	/**
	 * Get color based on proximity value - memoized for performance
	 */
	const color = React.useMemo(() => {
		if (proximity < 0.3) return "var(--cold)";
		if (proximity < 0.6) return "var(--warm)";
		return "var(--hot)";
	}, [proximity]);

	const fillStyle = React.useMemo(
		() => ({
			width: `${proximity * 100}%`,
			backgroundColor: color,
		}),
		[proximity, color]
	);

	return (
		<div
			className="proximity-container"
			role="meter"
			aria-label="Signal strength"
			aria-valuenow={Math.round(proximity * 100)}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div className="proximity-label">
				<span className="cold-indicator" aria-hidden="true">
					❄️
				</span>
				<span className="proximity-text">SIGNAL STRENGTH</span>
				<span className="hot-indicator" aria-hidden="true">
					🔥
				</span>
			</div>
			<div className="proximity-bar" aria-hidden="true">
				<div className="proximity-fill" style={fillStyle} />
			</div>
		</div>
	);
});

ProximityBar.displayName = "ProximityBar";

/**
 * Main Ghost component.
 * Renders the Ghost character with ASCII art, handles interactions,
 * displays clues, dialogue, and game state.
 *
 * @returns {JSX.Element} Ghost component
 *
 * @example
 * <Ghost />
 */
const Ghost = () => {
	const {
		gameState,
		extensionConnected,
		showHint,
		captureAndAnalyze,
		triggerDynamicPuzzle,
		startBackgroundChecks,
		enableAutonomousMode,
		verifyScreenshotProof,
		resetGame,
	} = useGhostGame();

	const sprite = GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle;
	const glowIntensity = Math.min(gameState.proximity * 30 + 5, 40);

	/**
	 * Handle click on Ghost - memoized for performance.
	 * Captures screen if idle, shows hint otherwise.
	 */
	const handleClick = useCallback(() => {
		if (gameState.state === "idle") {
			captureAndAnalyze();
		} else {
			showHint();
		}
	}, [gameState.state, captureAndAnalyze, showHint]);

	/**
	 * Handle drag to move window (Clippy-style) - memoized.
	 * Starts window dragging unless the click is on an interactive element.
	 */
	const handleDrag = useCallback(async (e) => {
		// Don't drag if clicking on buttons or other interactive elements
		const tagName = e.target.tagName.toUpperCase();
		if (tagName === "BUTTON" || tagName === "INPUT" || tagName === "A") {
			return;
		}
		try {
			await invoke("start_window_drag");
		} catch (err) {
			console.error("Failed to start drag:", err);
		}
	}, []);

	// Memoize sprite style to avoid object recreation
	const spriteStyle = useMemo(
		() => ({
			"--glow-intensity": `${glowIntensity}px`,
			"--glow-color":
				gameState.state === "celebrate"
					? "var(--celebrate-glow)"
					: "var(--ghost-glow)",
		}),
		[glowIntensity, gameState.state]
	);

	const [showingKeyInput, setShowingKeyInput] = useState(false);
	const [showingResetConfirm, setShowingResetConfirm] = useState(false);

	/**
	 * Handle successful API key set - refresh game state.
	 */
	const handleKeySet = useCallback(async () => {
		setShowingKeyInput(false);
		// Re-check API key status by calling backend
		try {
			const configured = await invoke("check_api_key");
			if (configured) {
				// Trigger a page reload to reinitialize with the new key
				window.location.reload();
			}
		} catch (err) {
			console.error("Failed to check API key:", err);
		}
	}, []);

	return (
		<div
			className="ghost-container"
			onMouseDown={handleDrag}
			role="application"
			aria-label="Ghost game interface"
		>
			{/* Ghost Sprite */}
			<div
				className={`ghost-sprite state-${gameState.state}`}
				onClick={handleClick}
				onKeyDown={(e) => {
					if (e.key === "Enter" || e.key === " ") {
						e.preventDefault();
						handleClick();
					}
				}}
				style={spriteStyle}
				role="button"
				tabIndex={0}
				aria-label={
					gameState.state === "idle"
						? "Click to analyze screen"
						: "Click for hint"
				}
			>
				<pre className="ascii-art" aria-hidden="true">
					{sprite}
				</pre>
			</div>

			{/* Proximity Indicator - Always visible */}
			<ProximityBar proximity={gameState.proximity} />

			{/* API Key Input - Show when not configured OR when user wants to change */}
			{(!gameState.apiKeyConfigured || showingKeyInput) && (
				<div className="status-section">
					<ApiKeyInput onKeySet={handleKeySet} />
					{showingKeyInput && (
						<button
							className="cancel-key-btn"
							onMouseDown={(e) => e.stopPropagation()}
							onClick={() => setShowingKeyInput(false)}
						>
							Cancel
						</button>
					)}
				</div>
			)}

			{/* Extension Connection Status - Show only when API key IS configured */}
			{gameState.apiKeyConfigured &&
				!showingKeyInput &&
				!extensionConnected && (
					<div className="status-section">
						<div className="connection-box">
							<div className="connection-header">
								🔗 Browser Not Connected
							</div>
							<p className="connection-text">
								Install the Chrome extension to enable browser
								tracking.
							</p>
							<button
								className="install-btn"
								onMouseDown={(e) => e.stopPropagation()}
								onClick={async () => {
									try {
										// Use the pre-configured command from capabilities
										const cmd = Command.create(
											"open-chrome-extensions"
										);
										await cmd.execute();
									} catch (err) {
										console.error(
											"Failed to open Chrome:",
											err
										);
										alert(
											'To install the extension:\n\n1. Open Chrome and go to: chrome://extensions\n2. Enable "Developer mode"\n3. Click "Load unpacked"\n4. Select the ghost-extension folder'
										);
									}
								}}
							>
								📦 Install Extension
							</button>
						</div>
					</div>
				)}

			{/* Game UI Section - Only show when everything is ready AND not configuring key */}
			{gameState.apiKeyConfigured &&
				extensionConnected &&
				!showingKeyInput && (
					<>
						{/* Show Game UI ONLY when NOT resetting */}
						{!showingResetConfirm ? (
							<>
								{/* Current Clue */}
								<div className="clue-box">
									<div className="clue-header">
										📜 CURRENT MYSTERY
										{gameState.is_sponsored && (
											<span
												style={{
													marginLeft: "8px",
													fontSize: "0.8em",
													background: "var(--accent)",
													color: "var(--bg-color)",
													padding: "2px 6px",
													borderRadius: "4px",
													fontWeight: "bold",
												}}
											>
												SPONSORED
											</span>
										)}
									</div>
									<p className="clue-text">
										{gameState.clue ||
											(gameState.puzzleId
												? "Loading puzzle..."
												: "Waiting for signal...")}
									</p>
									{gameState.puzzleId &&
										!gameState.hintAvailable && (
											<div className="hint-status">
												<span className="hint-charging">
													⏳ Hint charging...
												</span>
											</div>
										)}
								</div>

								{/* Dialogue Box */}
								{gameState.dialogue && (
									<div
										className={`dialogue-box state-${gameState.state}`}
									>
										{gameState.state === "searching" && (
											<div className="mode-indicator">
												🔍 Background Scan
											</div>
										)}
										{gameState.state === "thinking" && (
											<div className="mode-indicator">
												🔮 Consulting Oracle...
											</div>
										)}
										<TypewriterText
											text={gameState.dialogue}
											speed={25}
										/>
									</div>
								)}

								{/* Dynamic Puzzle Trigger */}
								{gameState.state === "idle" &&
									!gameState.puzzleId && (
										<div className="action-wrapper">
											<button
												className="action-btn"
												onMouseDown={(e) =>
													e.stopPropagation()
												}
												onClick={(e) => {
													e.stopPropagation();
													triggerDynamicPuzzle();
												}}
											>
												🌀 Investigate This Signal
											</button>
											<div className="helper-text">
												Generates a new mystery from
												this page
											</div>
										</div>
									)}

								{/* Prove Finding Button */}
								{gameState.puzzleId &&
									gameState.state !== "celebrate" && (
										<div className="action-wrapper">
											<button
												className="action-btn verify-btn"
												onMouseDown={(e) =>
													e.stopPropagation()
												}
												onClick={(e) => {
													e.stopPropagation();
													verifyScreenshotProof(); // Call verification
												}}
												style={{
													marginTop: "8px",
													backgroundColor:
														"var(--accent)",
												}}
											>
												📸 Prove Finding
											</button>
											<div className="helper-text">
												Verify you found the solution
											</div>
										</div>
									)}

								{/* Puzzle Counter */}
								<div className="puzzle-counter">
									Memory Fragment:{" "}
									{gameState.currentPuzzle + 1}
									/∞
								</div>
							</>
						) : (
							/* Reset Game Confirmation - Replaces Main UI */
							<div className="reset-confirm-box">
								<p>Reset all progress?</p>
								<div className="reset-actions">
									<button
										className="confirm-reset-btn"
										onMouseDown={(e) => e.stopPropagation()}
										onClick={() => {
											resetGame();
											setShowingResetConfirm(false);
										}}
									>
										Yes, Wipe Memory
									</button>
									<button
										className="cancel-reset-btn"
										onMouseDown={(e) => e.stopPropagation()}
										onClick={() =>
											setShowingResetConfirm(false)
										}
									>
										Cancel
									</button>
								</div>
							</div>
						)}
					</>
				)}

			{/* SYSTEM CONTROLS FOOTER - Always visible when key is configured */}
			{gameState.apiKeyConfigured && (
				<div className="system-controls">
					<div className="system-header">SYSTEM CONTROLS</div>
					<div className="system-controls-grid">
						{/* Top Row: Core Actions */}
						<button
							className={`system-btn change-key ${
								showingKeyInput ? "active" : ""
							}`}
							onMouseDown={(e) => e.stopPropagation()}
							onClick={() => {
								if (showingKeyInput) {
									setShowingKeyInput(false);
								} else {
									setShowingKeyInput(true);
									setShowingResetConfirm(false);
								}
							}}
						>
							{showingKeyInput ? "Close Key" : "Change Key"}
						</button>

						<button
							className={`system-btn danger ${
								showingResetConfirm ? "active" : ""
							}`}
							onMouseDown={(e) => e.stopPropagation()}
							onClick={() => {
								if (showingResetConfirm) {
									setShowingResetConfirm(false);
								} else {
									setShowingResetConfirm(true);
									setShowingKeyInput(false);
								}
							}}
						>
							{showingResetConfirm ? "Cancel" : "Reset Game"}
						</button>

						{/* Bottom Row: Dev/Tools */}
						<button
							className="system-btn dev-scan"
							onMouseDown={(e) => e.stopPropagation()}
							onClick={startBackgroundChecks}
							title="Scan Background Content"
						>
							Scan BG
						</button>

						<button
							className="system-btn dev-auto"
							onMouseDown={(e) => e.stopPropagation()}
							onClick={enableAutonomousMode}
							title="Enable Auto-Agent Mode"
						>
							Auto Mode
						</button>
					</div>
				</div>
			)}
		</div>
	);
};

export default Ghost;
