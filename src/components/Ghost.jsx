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
import PropTypes from "prop-types";
import { invoke } from "@tauri-apps/api/core";
import { useGhostGame } from "../hooks/useTauriCommands";
import ApiKeyInput from "./ApiKeyInput";
import SystemStatusBanner from "./SystemStatus";

/**
 * ASCII Art sprites for Ghost in different states.
 * @readonly
 * @type {Readonly<Record<string, string>>}
 */
const GHOST_SPRITES = Object.freeze({
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
});

/** Typewriter speed in milliseconds between characters */
const TYPEWRITER_SPEED = 25;

/** Maximum glow intensity in pixels */
const MAX_GLOW_INTENSITY = 40;

/** Base glow intensity in pixels */
const BASE_GLOW_INTENSITY = 5;

/** Glow intensity multiplier based on proximity */
const GLOW_PROXIMITY_MULTIPLIER = 30;

/** Style for sponsored badge - extracted to avoid inline object creation */
const SPONSORED_BADGE_STYLE = Object.freeze({
	marginLeft: "8px",
	fontSize: "0.8em",
	background: "var(--accent)",
	color: "var(--bg-color)",
	padding: "2px 6px",
	borderRadius: "4px",
	fontWeight: "bold",
});

/** Style for verify button - extracted to avoid inline object creation */
const VERIFY_BUTTON_STYLE = Object.freeze({
	marginTop: "8px",
	backgroundColor: "var(--accent)",
});

/**
 * Prevent event propagation - shared utility for drag handling.
 * @param {React.SyntheticEvent} e - Event to stop
 */
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
	const isMountedRef = useRef(true);

	useEffect(() => {
		isMountedRef.current = true;
		setDisplayed("");
		indexRef.current = 0;

		if (!text) return;

		const timer = setInterval(() => {
			if (!isMountedRef.current) {
				clearInterval(timer);
				return;
			}

			if (indexRef.current < text.length) {
				setDisplayed(text.slice(0, indexRef.current + 1));
				indexRef.current++;
			} else {
				clearInterval(timer);
			}
		}, speed);

		return () => {
			isMountedRef.current = false;
			clearInterval(timer);
		};
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

TypewriterText.propTypes = {
	text: PropTypes.string,
	speed: PropTypes.number,
};

TypewriterText.defaultProps = {
	text: "",
	speed: 30,
};

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
	const color = useMemo(() => {
		if (proximity < 0.3) return "var(--cold)";
		if (proximity < 0.6) return "var(--warm)";
		return "var(--hot)";
	}, [proximity]);

	const fillStyle = useMemo(
		() => ({
			width: `${proximity * 100}%`,
			backgroundColor: color,
		}),
		[proximity, color]
	);

	const percentValue = Math.round(proximity * 100);

	return (
		<div
			className="proximity-container"
			role="meter"
			aria-label="Signal strength"
			aria-valuenow={percentValue}
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

ProximityBar.propTypes = {
	proximity: PropTypes.number,
};

ProximityBar.defaultProps = {
	proximity: 0,
};

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
		systemStatus,
		companionBehavior,
		showHint,
		captureAndAnalyze,
		triggerDynamicPuzzle,
		startBackgroundChecks,
		enableAutonomousMode,
		verifyScreenshotProof,
		resetGame,
		generateAdaptivePuzzle,
	} = useGhostGame();

	const [showingKeyInput, setShowingKeyInput] = useState(false);
	const [showingResetConfirm, setShowingResetConfirm] = useState(false);
	const [extensionAccordionOpen, setExtensionAccordionOpen] = useState(false);

	// Memoize sprite lookup to avoid object reference on every render
	const sprite = useMemo(
		() => GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle,
		[gameState.state]
	);

	const glowIntensity = useMemo(
		() =>
			Math.min(
				gameState.proximity * GLOW_PROXIMITY_MULTIPLIER +
					BASE_GLOW_INTENSITY,
				MAX_GLOW_INTENSITY
			),
		[gameState.proximity]
	);

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
	 * Handle keyboard interaction on Ghost sprite.
	 */
	const handleSpriteKeyDown = useCallback(
		(e) => {
			if (e.key === "Enter" || e.key === " ") {
				e.preventDefault();
				handleClick();
			}
		},
		[handleClick]
	);

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

	/**
	 * Handle successful API key set - refresh game state.
	 */
	const handleKeySet = useCallback(async () => {
		setShowingKeyInput(false);
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

	/**
	 * Toggle API key input visibility.
	 */
	const toggleKeyInput = useCallback(() => {
		setShowingKeyInput((prev) => {
			if (!prev) {
				setShowingResetConfirm(false);
			}
			return !prev;
		});
	}, []);

	/**
	 * Toggle reset confirmation visibility.
	 */
	const toggleResetConfirm = useCallback(() => {
		setShowingResetConfirm((prev) => {
			if (!prev) {
				setShowingKeyInput(false);
			}
			return !prev;
		});
	}, []);

	/**
	 * Handle reset game confirmation.
	 */
	const handleConfirmReset = useCallback(() => {
		resetGame();
		setShowingResetConfirm(false);
	}, [resetGame]);

	/**
	 * Close reset confirmation.
	 */
	const closeResetConfirm = useCallback(() => {
		setShowingResetConfirm(false);
	}, []);

	/**
	 * Close key input.
	 */
	const closeKeyInput = useCallback(() => {
		setShowingKeyInput(false);
	}, []);

	// Derive aria-label for sprite based on state
	const spriteAriaLabel =
		gameState.state === "idle"
			? "Click to analyze screen"
			: "Click for hint";

	// Determine clue text to display
	const clueText = useMemo(() => {
		if (gameState.clue) return gameState.clue;
		return gameState.puzzleId
			? "Loading puzzle..."
			: "Waiting for signal...";
	}, [gameState.clue, gameState.puzzleId]);

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
				onKeyDown={handleSpriteKeyDown}
				style={spriteStyle}
				role="button"
				tabIndex={0}
				aria-label={spriteAriaLabel}
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
							type="button"
							className="cancel-key-btn"
							onMouseDown={stopPropagation}
							onClick={closeKeyInput}
						>
							Cancel
						</button>
					)}
				</div>
			)}

			{/* System Status Banner - Non-blocking, always visible when API key configured */}
			{gameState.apiKeyConfigured && !showingKeyInput && (
				<SystemStatusBanner
					status={systemStatus}
					extensionConnected={extensionConnected}
					isExpanded={extensionAccordionOpen}
					onToggleExpand={setExtensionAccordionOpen}
				/>
			)}

			{/* Game UI Section - Shows when API key is configured (extension is now optional) */}
			{gameState.apiKeyConfigured && !showingKeyInput && (
				<>
					{/* Show Game UI ONLY when NOT resetting */}
					{!showingResetConfirm ? (
						<>
							{/* Current Clue */}
							<div className="clue-box">
								<div className="clue-header">
									<span aria-hidden="true">📜</span> CURRENT
									MYSTERY
									{gameState.is_sponsored && (
										<span style={SPONSORED_BADGE_STYLE}>
											SPONSORED
										</span>
									)}
								</div>
								<p className="clue-text">{clueText}</p>
								{gameState.puzzleId &&
									!gameState.hintAvailable && (
										<div className="hint-status">
											<span
												className="hint-charging"
												aria-live="polite"
											>
												<span aria-hidden="true">
													⏳
												</span>{" "}
												Hint charging...
											</span>
										</div>
									)}
							</div>

							{/* Dialogue Box */}
							{gameState.dialogue && (
								<div
									className={`dialogue-box state-${gameState.state}`}
									role="status"
									aria-live="polite"
								>
									{gameState.state === "searching" && (
										<div className="mode-indicator">
											<span aria-hidden="true">🔍</span>{" "}
											Background Scan
										</div>
									)}
									{gameState.state === "thinking" && (
										<div className="mode-indicator">
											<span aria-hidden="true">🔮</span>{" "}
											Consulting Oracle...
										</div>
									)}
									<TypewriterText
										text={gameState.dialogue}
										speed={TYPEWRITER_SPEED}
									/>
								</div>
							)}

							{/* Companion Behavior Suggestion */}
							{companionBehavior && (
								<div
									className="companion-suggestion"
									role="status"
								>
									<div className="suggestion-message">
										<span aria-hidden="true">💭</span>{" "}
										{companionBehavior.suggestion}
									</div>
									{companionBehavior.behavior_type ===
										"puzzle" && (
										<button
											type="button"
											className="suggestion-action-btn"
											onMouseDown={stopPropagation}
											onClick={withStopPropagation(
												generateAdaptivePuzzle
											)}
										>
											<span aria-hidden="true">🎯</span>{" "}
											Create Puzzle
										</button>
									)}
								</div>
							)}

							{/* Dynamic Puzzle Trigger */}
							{gameState.state === "idle" &&
								!gameState.puzzleId && (
									<div
										className="action-wrapper"
										role="group"
										aria-label="Puzzle actions"
									>
										<button
											type="button"
											className="action-btn"
											onMouseDown={stopPropagation}
											onClick={withStopPropagation(
												triggerDynamicPuzzle
											)}
										>
											<span aria-hidden="true">🌀</span>{" "}
											Investigate This Signal
										</button>
										<button
											type="button"
											className="action-btn secondary"
											onMouseDown={stopPropagation}
											onClick={withStopPropagation(
												generateAdaptivePuzzle
											)}
										>
											<span aria-hidden="true">🧠</span>{" "}
											Puzzle From My Observations
										</button>
									</div>
								)}

							{/* Prove Finding Button */}
							{gameState.puzzleId &&
								gameState.state !== "celebrate" && (
									<div className="action-wrapper">
										<button
											type="button"
											className="action-btn verify-btn"
											onMouseDown={stopPropagation}
											onClick={withStopPropagation(
												verifyScreenshotProof
											)}
											style={VERIFY_BUTTON_STYLE}
										>
											<span aria-hidden="true">📸</span>{" "}
											Prove Finding
										</button>
									</div>
								)}
						</>
					) : (
						/* Reset Game Confirmation - Replaces Main UI */
						<div
							className="reset-confirm-box"
							role="alertdialog"
							aria-modal="true"
							aria-labelledby="reset-confirm-title"
						>
							<p id="reset-confirm-title">Reset all progress?</p>
							<div className="reset-actions">
								<button
									type="button"
									className="confirm-reset-btn"
									onMouseDown={stopPropagation}
									onClick={handleConfirmReset}
								>
									Yes, Wipe Memory
								</button>
								<button
									type="button"
									className="cancel-reset-btn"
									onMouseDown={stopPropagation}
									onClick={closeResetConfirm}
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
				<div
					className="system-controls"
					role="toolbar"
					aria-label="System controls"
				>
					<div className="system-header">SYSTEM CONTROLS</div>
					<div className="system-controls-grid">
						{/* Top Row: Core Actions */}
						<button
							type="button"
							className={`system-btn change-key ${showingKeyInput ? "active" : ""}`}
							onMouseDown={stopPropagation}
							onClick={toggleKeyInput}
							aria-pressed={showingKeyInput}
						>
							{showingKeyInput ? "Close Key" : "Change Key"}
						</button>

						<button
							type="button"
							className={`system-btn danger ${showingResetConfirm ? "active" : ""}`}
							onMouseDown={stopPropagation}
							onClick={toggleResetConfirm}
							aria-pressed={showingResetConfirm}
						>
							{showingResetConfirm ? "Cancel" : "Reset Game"}
						</button>

						{/* Bottom Row: Dev/Tools */}
						<button
							type="button"
							className="system-btn dev-scan"
							onMouseDown={stopPropagation}
							onClick={startBackgroundChecks}
							title="Scan Background Content"
							aria-label="Scan background content"
						>
							Scan BG
						</button>

						<button
							type="button"
							className="system-btn dev-auto"
							onMouseDown={stopPropagation}
							onClick={enableAutonomousMode}
							title="Enable Auto-Agent Mode"
							aria-label="Enable auto-agent mode"
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
