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
import {
	DialogueFeedback,
	StuckButton,
	IntelligentModeSettings,
} from "./FeedbackButtons";

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
		// HITL Feedback (Chapter 13)
		submitFeedback,
		reportStuck,
		// Intelligent Mode Settings
		getIntelligentMode,
		setIntelligentMode,
		setReflectionMode,
		setGuardrailsMode,
		setAppMode,
		autonomySettings,
		setAutonomySettings,
	} = useGhostGame();

	const [activeDialog, setActiveDialog] = useState(null); // extension | privacy | key | reset
	const [intelligentSettings, setIntelligentSettings] = useState(null);
	const [puzzleStartTime, setPuzzleStartTime] = useState(null);

	// Privacy / consent (required for background monitoring + screenshots)
	const [privacy, setPrivacy] = useState({
		settings: null,
		notice: "",
	});
	const [privacyForm, setPrivacyForm] = useState({
		captureConsent: false,
		aiConsent: false,
		noticeAck: false,
	});

	// Fetch intelligent mode settings on mount
	useEffect(() => {
		if (gameState.apiKeyConfigured) {
			getIntelligentMode?.().then(setIntelligentSettings);
		}
	}, [gameState.apiKeyConfigured, getIntelligentMode]);

	const hasFullPrivacyConsent = useMemo(() => {
		if (!privacy.settings) return false;
		return (
			privacy.settings.capture_consent &&
			privacy.settings.ai_analysis_consent &&
			privacy.settings.privacy_notice_acknowledged
		);
	}, [privacy.settings]);

	// Load privacy notice + settings (one-time flow).
	useEffect(() => {
		if (!gameState.apiKeyConfigured) return;

		const loadPrivacy = async () => {
			try {
				const settings = await invoke("get_privacy_settings");
				const notice = await invoke("get_privacy_notice");

				setPrivacy((prev) => ({
					...prev,
					settings,
					notice,
				}));

				setPrivacyForm({
					captureConsent: !!settings.capture_consent,
					aiConsent: !!settings.ai_analysis_consent,
					noticeAck: !!settings.privacy_notice_acknowledged,
				});

				// Auto-open privacy dialog if not acknowledged (first time user)
				if (!settings.privacy_notice_acknowledged) {
					setActiveDialog("privacy");
				}
			} catch (err) {
				console.error("Failed to load privacy settings:", err);
			}
		};

		loadPrivacy();
	}, [gameState.apiKeyConfigured]);

	// Track puzzle start time
	useEffect(() => {
		if (gameState.puzzleId && !puzzleStartTime) {
			setPuzzleStartTime(Date.now());
		} else if (!gameState.puzzleId) {
			setPuzzleStartTime(null);
		}
	}, [gameState.puzzleId, puzzleStartTime]);

	// Handlers for intelligent mode toggles
	const handleToggleIntelligent = useCallback(
		async (enabled) => {
			const result = await setIntelligentMode?.(enabled);
			if (result) setIntelligentSettings(result);
		},
		[setIntelligentMode]
	);

	const handleToggleReflection = useCallback(
		async (enabled) => {
			const result = await setReflectionMode?.(enabled);
			if (result) setIntelligentSettings(result);
		},
		[setReflectionMode]
	);

	const handleToggleGuardrails = useCallback(
		async (enabled) => {
			const result = await setGuardrailsMode?.(enabled);
			if (result) setIntelligentSettings(result);
		},
		[setGuardrailsMode]
	);

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

	const savePrivacySettings = useCallback(async () => {
		try {
			const updated = await invoke("update_privacy_settings", {
				captureConsent: privacyForm.captureConsent,
				aiAnalysisConsent: privacyForm.aiConsent,
				privacyNoticeAcknowledged: privacyForm.noticeAck,
			});
			setPrivacy((prev) => ({
				...prev,
				settings: updated,
			}));
			setActiveDialog(null);
		} catch (err) {
			console.error("Failed to update privacy settings:", err);
		}
	}, [privacyForm.aiConsent, privacyForm.captureConsent, privacyForm.noticeAck]);

	const openPrivacyModal = useCallback(() => {
		// Reset the form to last saved settings each time we open.
		if (privacy.settings) {
			setPrivacyForm({
				captureConsent: !!privacy.settings.capture_consent,
				aiConsent: !!privacy.settings.ai_analysis_consent,
				noticeAck: !!privacy.settings.privacy_notice_acknowledged,
			});
		}
		setActiveDialog("privacy");
	}, [privacy.settings]);

	// Memoized handlers for privacy form to avoid recreating on every render
	const handleCaptureConsentChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, captureConsent: e.target.checked }));
	}, []);
	const handleAiConsentChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, aiConsent: e.target.checked }));
	}, []);
	const handleNoticeAckChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, noticeAck: e.target.checked }));
	}, []);

	/**
	 * Handle click on Ghost - memoized for performance.
	 * Captures screen if idle, shows hint otherwise.
	 */
	const handleClick = useCallback(() => {
		if (gameState.state === "idle") {
			if (!hasFullPrivacyConsent) {
				openPrivacyModal();
				return;
			}
			captureAndAnalyze();
		} else {
			showHint();
		}
	}, [captureAndAnalyze, gameState.state, hasFullPrivacyConsent, openPrivacyModal, showHint]);

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

	const handleVerifyProof = useCallback(() => {
		if (!hasFullPrivacyConsent) {
			openPrivacyModal();
			return;
		}
		verifyScreenshotProof();
	}, [hasFullPrivacyConsent, openPrivacyModal, verifyScreenshotProof]);

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
		setActiveDialog(null);
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

	const openDialog = useCallback((type) => {
		setActiveDialog(type);
	}, []);

	const closeDialog = useCallback(() => {
		setActiveDialog(null);
	}, []);

	const handleConfirmReset = useCallback(() => {
		resetGame();
		setActiveDialog(null);
	}, [resetGame]);

	// Derive aria-label for sprite based on state - memoized
	const spriteAriaLabel = useMemo(
		() =>
			gameState.state === "idle"
				? "Click to analyze screen"
				: "Click for hint",
		[gameState.state]
	);

	// Memoize mode derivations to avoid recalculation
	const currentMode = useMemo(
		() => systemStatus?.currentMode || "game",
		[systemStatus?.currentMode]
	);
	const isCompanionMode = currentMode === "companion";

	const toggleMode = useCallback(() => {
		if (!setAppMode) return;
		setAppMode(isCompanionMode ? "game" : "companion");
	}, [isCompanionMode, setAppMode]);

	const ensureGameMode = useCallback(async () => {
		if (isCompanionMode && setAppMode) {
			await setAppMode("game", { persist: false });
		}
	}, [isCompanionMode, setAppMode]);

	const handleGenerateAdaptivePuzzle = useCallback(async () => {
		await ensureGameMode();
		await generateAdaptivePuzzle();
	}, [ensureGameMode, generateAdaptivePuzzle]);

	const handleTriggerDynamicPuzzle = useCallback(async () => {
		await ensureGameMode();
		await triggerDynamicPuzzle();
	}, [ensureGameMode, triggerDynamicPuzzle]);

	const toggleAutoPuzzle = useCallback(() => {
		setAutonomySettings?.((prev) => ({
			...prev,
			autoPuzzleFromCompanion: !prev.autoPuzzleFromCompanion,
		}));
	}, [setAutonomySettings]);

	// Determine clue text to display
	const clueText = useMemo(() => {
		if (gameState.clue) return gameState.clue;
		return gameState.puzzleId
			? "Loading puzzle..."
			: "Waiting for signal...";
	}, [gameState.clue, gameState.puzzleId]);

	useEffect(() => {
		// Wait for initial check (null)
		if (gameState.apiKeyConfigured === null) return;

		// If missing, open dialog (enforce requirement)
		if (gameState.apiKeyConfigured === false) {
			if (!activeDialog) openDialog("key");
		}
		// NOTE: Do NOT auto-close if true. The user might have manually opened the dialog to change the key.
		// Closing is handled by handleKeySet or the Close button.
	}, [activeDialog, gameState.apiKeyConfigured, openDialog]);

	return (
		<div
			className="ghost-container"
			onMouseDown={handleDrag}
			role="application"
			aria-label="Ghost game interface"
		>
			{activeDialog && (
				<div className="ghost-modal-overlay" onMouseDown={closeDialog}>
					<div
						className={`ghost-modal ${activeDialog === "reset" ? "danger" : ""}`}
						role="dialog"
						aria-modal="true"
						aria-label="Ghost dialog"
						onMouseDown={stopPropagation}
					>
						{activeDialog === "privacy" && (
							<>
								<div className="ghost-modal-title">Privacy & Consent</div>
								<pre className="privacy-notice">{privacy.notice}</pre>

								<label className="privacy-checkbox">
									<input
										type="checkbox"
										checked={privacyForm.captureConsent}
										onChange={handleCaptureConsentChange}
									/>
									Allow screen capture (enables screenshots)
								</label>
								<label className="privacy-checkbox">
									<input
										type="checkbox"
										checked={privacyForm.aiConsent}
										onChange={handleAiConsentChange}
									/>
									Allow AI analysis (enables Companion monitoring)
								</label>
								<label className="privacy-checkbox">
									<input
										type="checkbox"
										checked={privacyForm.noticeAck}
										onChange={handleNoticeAckChange}
									/>
									I have read this notice
								</label>

								<div className="ghost-modal-actions">
									<button
										type="button"
										className="ghost-modal-btn"
										disabled={!privacyForm.noticeAck}
										onMouseDown={stopPropagation}
										onClick={savePrivacySettings}
										title={
											privacyForm.captureConsent && privacyForm.aiConsent
												? ""
												: "Without both consents, screenshots and background monitoring remain off"
										}
									>
										Save
									</button>
									<button
										type="button"
										className="ghost-modal-btn secondary"
										onMouseDown={stopPropagation}
										onClick={closeDialog}
									>
										Close
									</button>
								</div>
							</>
						)}

						{activeDialog === "extension" && (
							<>
								<div className="ghost-modal-title">Extension</div>
								<SystemStatusBanner
									status={systemStatus}
									extensionConnected={extensionConnected}
									flat
								/>
								<div className="ghost-modal-actions">
									<button
										type="button"
										className="ghost-modal-btn secondary"
										onMouseDown={stopPropagation}
										onClick={closeDialog}
									>
										Close
									</button>
								</div>
							</>
						)}

						{activeDialog === "key" && (
							<>
								<div className="ghost-modal-title">AI Configuration</div>
								<ApiKeyInput
									onKeySet={handleKeySet}
									apiKeySource={systemStatus.apiKeySource}
								/>
								<div className="ghost-modal-actions">
									<button
										type="button"
										className="ghost-modal-btn secondary"
										onMouseDown={stopPropagation}
										onClick={closeDialog}
									>
										Close
									</button>
								</div>
							</>
						)}

						{activeDialog === "reset" && (
							<>
								<div className="ghost-modal-title">
									Reset All Progress?
								</div>
								<div
									className="reset-confirm-box"
									role="alertdialog"
									aria-modal="true"
								>
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
											onClick={closeDialog}
										>
											Cancel
										</button>
									</div>
								</div>
							</>
						)}
					</div>
				</div>
			)}

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

			{/* Game UI Section - Shows when API key is configured (extension is now optional) */}
			{gameState.apiKeyConfigured && (
				<>
					{/* Current Clue */}
					<div className="clue-box">
						<div className="clue-header">
							<div className="clue-header-left">
								<span aria-hidden="true">📜</span>{" "}
								{isCompanionMode ? "COMPANION" : "CURRENT"}{" "}
								{isCompanionMode ? "MODE" : "MYSTERY"}
								{gameState.is_sponsored && (
									<span style={SPONSORED_BADGE_STYLE}>
										SPONSORED
									</span>
								)}
							</div>
							<div className="mode-toggle-bar">
								<button
									type="button"
									className={`mode-toggle-btn ${isCompanionMode ? "active" : ""}`}
									onMouseDown={stopPropagation}
									onClick={toggleMode}
									aria-pressed={isCompanionMode}
								>
									{isCompanionMode ? "Companion" : "Game"}
								</button>
								<button
									type="button"
									disabled={!isCompanionMode}
									className={`mode-toggle-btn ${autonomySettings?.autoPuzzleFromCompanion ? "active" : ""}`}
									onMouseDown={stopPropagation}
									onClick={toggleAutoPuzzle}
									aria-pressed={!!autonomySettings?.autoPuzzleFromCompanion}
									title="Auto-create puzzles in Companion mode"
								>
									Auto
								</button>
							</div>
						</div>
						<p className="clue-text">{clueText}</p>
						{gameState.puzzleId &&
							!gameState.hintAvailable && (
								<div className="hint-status">
									<span
										className="hint-charging"
										aria-live="polite"
									>
										<span aria-hidden="true">⏳</span>{" "}
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
							<div className="dialogue-scroll-area">
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
							{/* HITL Feedback Buttons */}
							{gameState.state === "idle" && (
								<DialogueFeedback
									content={gameState.dialogue}
									onFeedback={submitFeedback}
								/>
							)}
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
							{companionBehavior.behavior_type === "puzzle" && (
								<button
									type="button"
									className="suggestion-action-btn"
									onMouseDown={stopPropagation}
									onClick={withStopPropagation(handleGenerateAdaptivePuzzle)}
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
									onClick={withStopPropagation(handleTriggerDynamicPuzzle)}
								>
									<span aria-hidden="true">🌀</span>{" "}
									Investigate This Signal
								</button>
								<button
									type="button"
									className="action-btn secondary"
									onMouseDown={stopPropagation}
									onClick={withStopPropagation(handleGenerateAdaptivePuzzle)}
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
									onClick={withStopPropagation(handleVerifyProof)}
									style={VERIFY_BUTTON_STYLE}
								>
									<span aria-hidden="true">📸</span>{" "}
									Prove Finding
								</button>
							</div>
						)}

					{/* I'm Stuck Button - HITL Escalation */}
					{gameState.puzzleId &&
						gameState.state === "idle" && (
							<StuckButton
								onStuck={reportStuck}
								puzzleStartTime={puzzleStartTime}
							/>
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

					{/* Intelligent Mode Settings */}
					<IntelligentModeSettings
						settings={intelligentSettings}
						onToggleIntelligent={handleToggleIntelligent}
						onToggleReflection={handleToggleReflection}
						onToggleGuardrails={handleToggleGuardrails}
					/>

					<div className="system-controls-grid">
						<button
							type="button"
							className="system-btn secondary"
							onMouseDown={stopPropagation}
							onClick={() => openDialog("extension")}
						>
							Extension
						</button>

						<button
							type="button"
							className="system-btn secondary"
							onMouseDown={stopPropagation}
							onClick={openPrivacyModal}
						>
							Privacy
						</button>

						<button
							type="button"
							className="system-btn change-key"
							onMouseDown={stopPropagation}
							onClick={() => openDialog("key")}
						>
							Change Key
						</button>

						<button
							type="button"
							className="system-btn danger"
							onMouseDown={stopPropagation}
							onClick={() => openDialog("reset")}
						>
							Reset Game
						</button>
					</div>
				</div>
			)}
		</div>
	);
};

export default Ghost;
