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
import { safeInvoke } from "../utils/data";
import { useGhostGame } from "../hooks/useTauriCommands";
import { useActionManagement } from "../hooks/useActionManagement";
import ApiKeyInput from "./ApiKeyInput";
import SystemStatusBanner from "./SystemStatus";
import {
	DialogueFeedback,
	StuckButton,
	IntelligentModeSettings,
	SystemControlsAccordion,
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
	const [statusExpanded, setStatusExpanded] = useState(false);
	const [quickAsk, setQuickAsk] = useState({
		prompt: "",
		response: "",
		isLoading: false,
		error: "",
	});

	// Privacy / consent (required for background monitoring + screenshots)
	const [privacy, setPrivacy] = useState({
		settings: null,
		notice: "",
	});
	const [privacyForm, setPrivacyForm] = useState({
		captureConsent: false,
		aiConsent: false,
		noticeAck: false,
		readOnly: false,
		autonomyLevel: "observer",
	});
	const {
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
		setEditingParam,
		approveAction,
		denyAction,
		approvePreview,
		denyPreview,
		editPreviewParam,
		undoAction,
		redoAction,
		fetchActionHistory,
		closeActionHistory,
		openSandboxSettings,
		closeSandboxSettings,
		setTrustLevel,
		toggleShellCategory,
		addReadPath,
		removeReadPath,
		addWritePath,
		removeWritePath,
	} = useActionManagement(
		privacy.settings?.autonomy_level || "observer",
		!!gameState.apiKeyConfigured
	);

	const [readPathInput, setReadPathInput] = useState("");
	const [writePathInput, setWritePathInput] = useState("");
	const [sandboxPathError, setSandboxPathError] = useState("");

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
			privacy.settings.privacy_notice_acknowledged &&
			!privacy.settings.read_only_mode
		);
	}, [privacy.settings]);

	// Load privacy notice + settings (one-time flow).
	useEffect(() => {
		if (!gameState.apiKeyConfigured) return;

		const loadPrivacy = async () => {
			const settings = await safeInvoke("get_privacy_settings", {}, null);
			const notice = await safeInvoke("get_privacy_notice", {}, "");

			if (settings) {
				setPrivacy((prev) => ({
					...prev,
					settings,
					notice,
				}));

				setPrivacyForm({
					captureConsent: !!settings.capture_consent,
					aiConsent: !!settings.ai_analysis_consent,
					noticeAck: !!settings.privacy_notice_acknowledged,
					readOnly: !!settings.read_only_mode,
					autonomyLevel: settings.autonomy_level || "observer",
				});

				// Auto-open privacy dialog if not acknowledged (first time user)
				if (!settings.privacy_notice_acknowledged) {
					setActiveDialog("privacy");
				}
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
		const updated = await safeInvoke(
			"update_privacy_settings",
			{
				capture_consent: privacyForm.captureConsent,
				ai_analysis_consent: privacyForm.aiConsent,
				privacy_notice_acknowledged: privacyForm.noticeAck,
				read_only_mode: privacyForm.readOnly,
				autonomy_level: privacyForm.autonomyLevel,
			},
			null
		);

		if (updated) {
			setPrivacy((prev) => ({
				...prev,
				settings: updated,
			}));
			setActiveDialog(null);
		}
	}, [
		privacyForm.aiConsent,
		privacyForm.captureConsent,
		privacyForm.noticeAck,
		privacyForm.readOnly,
		privacyForm.autonomyLevel,
	]);

	const openPrivacyModal = useCallback(() => {
		// Reset the form to last saved settings each time we open.
		if (privacy.settings) {
			setPrivacyForm({
				captureConsent: !!privacy.settings.capture_consent,
				aiConsent: !!privacy.settings.ai_analysis_consent,
				noticeAck: !!privacy.settings.privacy_notice_acknowledged,
				readOnly: !!privacy.settings.read_only_mode,
				autonomyLevel: privacy.settings.autonomy_level || "observer",
			});
		}
		setActiveDialog("privacy");
	}, [privacy.settings]);

	// Memoized handlers for privacy form to avoid recreating on every render
	const handleCaptureConsentChange = useCallback((e) => {
		setPrivacyForm((prev) => ({
			...prev,
			captureConsent: e.target.checked,
		}));
	}, []);
	const handleAiConsentChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, aiConsent: e.target.checked }));
	}, []);
	const handleNoticeAckChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, noticeAck: e.target.checked }));
	}, []);
	const handleReadOnlyChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, readOnly: e.target.checked }));
	}, []);
	const handleAutonomyLevelChange = useCallback((e) => {
		setPrivacyForm((prev) => ({ ...prev, autonomyLevel: e.target.value }));
	}, []);

	const handleShowActionHistory = useCallback(() => {
		fetchActionHistory();
	}, [fetchActionHistory]);

	const handleShowSandboxSettings = useCallback(() => {
		openSandboxSettings();
	}, [openSandboxSettings]);

	const handleSetTrustLevel = useCallback(async (level) => {
		await setTrustLevel(level);
	}, [setTrustLevel]);

	const handleToggleShellCategory = useCallback(async (category, enabled) => {
		await toggleShellCategory(category, enabled);
	}, [toggleShellCategory]);

	const handleAddReadPath = useCallback(async () => {
		const path = readPathInput.trim();
		if (!path) return;
		const result = await addReadPath(path);
		if (result?.success) {
			setReadPathInput("");
			setSandboxPathError("");
		} else {
			setSandboxPathError(result?.error || "Failed to add read path.");
		}
	}, [addReadPath, readPathInput]);

	const handleAddWritePath = useCallback(async () => {
		const path = writePathInput.trim();
		if (!path) return;
		const result = await addWritePath(path);
		if (result?.success) {
			setWritePathInput("");
			setSandboxPathError("");
		} else {
			setSandboxPathError(result?.error || "Failed to add write path.");
		}
	}, [addWritePath, writePathInput]);

	const handleRemoveReadPath = useCallback(async (path) => {
		const result = await removeReadPath(path);
		if (result?.success) {
			setSandboxPathError("");
		} else {
			setSandboxPathError(result?.error || "Failed to remove read path.");
		}
	}, [removeReadPath]);

	const handleRemoveWritePath = useCallback(async (path) => {
		const result = await removeWritePath(path);
		if (result?.success) {
			setSandboxPathError("");
		} else {
			setSandboxPathError(result?.error || "Failed to remove write path.");
		}
	}, [removeWritePath]);

	const handleApproveAction = useCallback(async (actionId) => {
		await approveAction(actionId);
	}, [approveAction]);

	const handleDenyAction = useCallback(async (actionId) => {
		await denyAction(actionId);
	}, [denyAction]);

	const handleApprovePreview = useCallback(async () => {
		await approvePreview();
	}, [approvePreview]);

	const handleDenyPreview = useCallback(async (reason) => {
		await denyPreview(reason);
	}, [denyPreview]);

	const handleEditPreviewParam = useCallback(async (paramName, value) => {
		await editPreviewParam(paramName, value);
	}, [editPreviewParam]);

	const handleUndo = useCallback(async () => {
		await undoAction();
	}, [undoAction]);

	const handleRedo = useCallback(async () => {
		await redoAction();
	}, [redoAction]);

	const toggleReadOnly = useCallback(async () => {
		if (!privacy.settings) return;
		const updated = await safeInvoke(
			"update_privacy_settings",
			{
				capture_consent: privacy.settings.capture_consent,
				ai_analysis_consent: privacy.settings.ai_analysis_consent,
				privacy_notice_acknowledged:
					privacy.settings.privacy_notice_acknowledged,
				read_only_mode: !privacy.settings.read_only_mode,
				autonomy_level: privacy.settings.autonomy_level || "observer",
			},
			null
		);

		if (updated) {
			setPrivacy((prev) => ({ ...prev, settings: updated }));
			setPrivacyForm((prev) => ({
				...prev,
				readOnly: !!updated.read_only_mode,
			}));
		}
	}, [privacy.settings]);

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
	}, [
		captureAndAnalyze,
		gameState.state,
		hasFullPrivacyConsent,
		openPrivacyModal,
		showHint,
	]);

	const handleAnalyze = useCallback(() => {
		if (!hasFullPrivacyConsent) {
			openPrivacyModal();
			return;
		}
		captureAndAnalyze();
	}, [captureAndAnalyze, hasFullPrivacyConsent, openPrivacyModal]);

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
		// Using invoke directly for simple void commands with no return needed is fine,
		// but for consistency we can use safeInvoke or keep try/catch if we want to swallow errors silently.
		// safeInvoke logs errors which is what we want.
		await safeInvoke("start_window_drag");
	}, []);

	/**
	 * Handle successful API key set - refresh game state.
	 */
	const handleKeySet = useCallback(async () => {
		setActiveDialog(null);
		const configured = await safeInvoke("check_api_key", {}, false);
		if (configured) {
			// Trigger a page reload to reinitialize with the new key
			window.location.reload();
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

	const isBusy = useMemo(
		() =>
			gameState.state === "thinking" ||
			gameState.state === "searching" ||
			gameState.state === "celebrate",
		[gameState.state]
	);

	const canAnalyze = useMemo(
		() => gameState.apiKeyConfigured && !isBusy,
		[gameState.apiKeyConfigured, isBusy]
	);

	const canHint = useMemo(
		() => !!gameState.puzzleId && !!gameState.hintAvailable && !isBusy,
		[gameState.puzzleId, gameState.hintAvailable, isBusy]
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

	const handleQuickAskChange = useCallback((e) => {
		setQuickAsk((prev) => ({ ...prev, prompt: e.target.value, error: "", response: "" }));
	}, []);

	const handleQuickAskSubmit = useCallback(
		async (e) => {
			e.preventDefault();
			e.stopPropagation();
			if (!quickAsk.prompt.trim()) {
				setQuickAsk((prev) => ({
					...prev,
					error: "Please enter a question.",
				}));
				return;
			}

			setQuickAsk((prev) => ({ ...prev, isLoading: true, error: "", response: "" }));
			try {
				const response = await invoke("quick_ask", {
					prompt: quickAsk.prompt.trim(),
				});
				setQuickAsk((prev) => ({
					...prev,
					response,
					isLoading: false,
					prompt: "", // Clear prompt on success
				}));
			} catch (err) {
				const message = typeof err === "string" ? err : err?.message || "Quick ask failed";
				setQuickAsk((prev) => ({
					...prev,
					isLoading: false,
					error: message.includes("API") || message.includes("key") 
						? "API key required. Configure it first."
						: "Quick ask failed. Try again.",
				}));
			}
		},
		[quickAsk.prompt]
	);

	// Determine clue text to display - context-aware based on current state
	const clueText = useMemo(() => {
		// If there's an active clue, show it
		if (gameState.clue) return gameState.clue;
		
		// If loading a puzzle
		if (gameState.puzzleId) return "Loading puzzle...";
		
		// In Companion Auto mode, show status-based messages
		if (isCompanionMode && autonomySettings?.autoPuzzleFromCompanion) {
			if (privacy.settings?.read_only_mode) {
				return "Read-only mode active. Toggle 🛡️ to enable watching.";
			}
			if (!hasFullPrivacyConsent) {
				return "Privacy consent needed. Click Privacy in settings.";
			}
			if (extensionConnected) {
				return "Connected. Browse the web to generate mysteries...";
			}
			return "Monitoring your screen for interesting content...";
		}
		
		// Game mode or manual companion mode
		if (extensionConnected) {
			return "Extension connected. Browse a page to begin.";
		}
		
		return "Waiting for signal... browse the web to begin.";
	}, [gameState.clue, gameState.puzzleId, isCompanionMode, autonomySettings?.autoPuzzleFromCompanion, privacy.settings?.read_only_mode, hasFullPrivacyConsent, extensionConnected]);

	// Compute display dialogue - use clueText for initial/context messages, otherwise gameState.dialogue
	const displayDialogue = useMemo(() => {
		// If there's no dialogue at all, use clueText
		if (!gameState.dialogue) return clueText;
		
		// If dialogue is the initial placeholder and we have a more contextual clueText
		const initialPlaceholder = "Waiting for signal... browse the web to begin.";
		if (gameState.dialogue === initialPlaceholder && !gameState.puzzleId) {
			return clueText;
		}
		
		// Otherwise use the actual dialogue
		return gameState.dialogue;
	}, [gameState.dialogue, gameState.puzzleId, clueText]);

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
			className={`ghost-container ${privacy.settings?.read_only_mode ? "read-only-mode" : ""} ${isCompanionMode ? "companion-mode" : "game-mode"}`}
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
								<div className="ghost-modal-title">
									Privacy & Consent
								</div>
								<pre className="privacy-notice">
									{privacy.notice}
								</pre>

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
									Allow AI analysis (enables Companion
									monitoring)
								</label>
								<label className="privacy-checkbox">
									<input
										type="checkbox"
										checked={privacyForm.noticeAck}
										onChange={handleNoticeAckChange}
									/>
									I have read this notice
								</label>
									<label className="privacy-checkbox">
										<input
											type="checkbox"
											checked={privacyForm.readOnly}
											onChange={handleReadOnlyChange}
										/>
										Read-only mode (disable capture & automation)
									</label>

								{/* Autonomy Level Selector */}
								<div className="autonomy-level-section">
									<div className="autonomy-level-label">Action Autonomy Level:</div>
									<select
										className="autonomy-level-select"
										value={privacyForm.autonomyLevel}
										onChange={handleAutonomyLevelChange}
										disabled={privacyForm.readOnly}
									>
										<option value="observer">👁️ Observer - Watch only, no actions</option>
										<option value="suggester">💬 Suggester - Proposes actions, you confirm each</option>
										<option value="supervised">⚡ Supervised - Auto-executes safe, confirms risky</option>
										<option value="autonomous">🤖 Autonomous - Full control within guardrails</option>
									</select>
									<div className="autonomy-level-hint">
										{privacyForm.readOnly 
											? "Read-only mode overrides autonomy level"
											: privacyForm.autonomyLevel === "observer"
												? "Ghost will only observe and narrate"
												: privacyForm.autonomyLevel === "suggester"
													? "Ghost will ask before any browser action"
													: privacyForm.autonomyLevel === "supervised"
														? "Ghost will auto-execute safe actions, confirm risky ones"
														: "⚠️ Full autonomy - Ghost will execute all actions within guardrails"
										}
									</div>
									{privacyForm.autonomyLevel === "autonomous" && (
										<div className="autonomy-warning">
											<span aria-hidden="true">⚠️</span>
											<strong>Warning:</strong> Autonomous mode gives the Ghost full control 
											to execute browser actions without confirmation. Safety guardrails 
											and watchdog monitoring remain active.
										</div>
									)}
								</div>

								<div className="ghost-modal-actions">
									<button
										type="button"
										className="ghost-modal-btn"
										disabled={!privacyForm.noticeAck}
										onMouseDown={stopPropagation}
										onClick={savePrivacySettings}
										title={
											privacyForm.captureConsent &&
											privacyForm.aiConsent
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
								<div className="ghost-modal-title">
									Extension
								</div>
								<SystemStatusBanner
									status={systemStatus}
									extensionConnected={extensionConnected}
									readOnlyMode={!!privacy.settings?.read_only_mode}
									hasConsent={hasFullPrivacyConsent}
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
								<div className="ghost-modal-title">
									AI Configuration
								</div>
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

			{/* Pending Action Confirmation Panel */}
			{pendingActions.length > 0 && (
				<div className="pending-actions-panel" role="alertdialog" aria-label="Pending actions">
					<div className="pending-actions-header">
						<span aria-hidden="true">⚡</span> Action Confirmation
						<span className="pending-count">{pendingActions.length}</span>
					</div>
					{pendingActions.map((action) => (
						<div key={action.id} className={`pending-action-item risk-${action.risk_level}`}>
							<div className="action-description">{action.description}</div>
							{action.reason && (
								<div className="action-reason">Reason: {action.reason}</div>
							)}
							<div className="action-buttons">
								<button
									type="button"
									className="action-approve-btn"
									onClick={() => handleApproveAction(action.id)}
									onMouseDown={stopPropagation}
								>
									✓ Allow
								</button>
								<button
									type="button"
									className="action-deny-btn"
									onClick={() => handleDenyAction(action.id)}
									onMouseDown={stopPropagation}
								>
									✕ Deny
								</button>
							</div>
						</div>
					))}
				</div>
			)}

			{/* Action Preview Panel - Rich streaming preview with editing */}
			{actionPreview && (
				<div className="action-preview-panel" role="alertdialog" aria-label="Action preview">
					<div className="preview-header">
						<span className="preview-icon" aria-hidden="true">👁️</span>
						<span className="preview-title">Action Preview</span>
						<span className={`preview-risk risk-${actionPreview.action?.risk_level?.toLowerCase() || 'low'}`}>
							{actionPreview.action?.risk_level || 'Low'} Risk
						</span>
					</div>
					
					<div className="preview-state">
						{actionPreview.state === 'loading' && <span className="loading-indicator">⏳ Loading preview...</span>}
						{actionPreview.state === 'streaming' && (
							<div className="streaming-progress">
								<div className="progress-bar" style={{ width: `${(actionPreview.progress || 0) * 100}%` }} />
								<span className="progress-text">{Math.round((actionPreview.progress || 0) * 100)}%</span>
							</div>
						)}
						{actionPreview.state === 'ready' && <span className="ready-indicator">✓ Ready for approval</span>}
						{actionPreview.state === 'editing' && <span className="editing-indicator">✏️ Editing parameters...</span>}
					</div>

					<div className="preview-action-details">
						<div className="action-type">{actionPreview.action?.action_type}</div>
						<div className="action-desc">{actionPreview.action?.description}</div>
					</div>

					{/* Visual Preview */}
					{actionPreview.visual_preview && (
						<div className="visual-preview">
							{actionPreview.visual_preview.preview_type === 'url_card' && (
								<div className="url-card">
									<span className="url-icon">🔗</span>
									<span className="url-text">{actionPreview.visual_preview.content}</span>
								</div>
							)}
							{actionPreview.visual_preview.preview_type === 'text_selection' && (
								<div className="text-selection-preview">
									<span className="highlight-icon">✨</span>
									<mark className="preview-highlight">{actionPreview.visual_preview.content}</mark>
								</div>
							)}
						</div>
					)}

					{/* Editable Parameters */}
					{actionPreview.editable_params && Object.keys(actionPreview.editable_params).length > 0 && (
						<div className="editable-params">
							<div className="params-header">Edit before executing:</div>
							{Object.entries(actionPreview.editable_params).map(([name, param]) => (
								<div key={name} className="param-row">
									<label className="param-label">{param.label}:</label>
									{editingParam === name ? (
										<input
											type={param.param_type === 'url' ? 'url' : param.param_type === 'number' ? 'number' : 'text'}
											className="param-input"
											defaultValue={param.value}
											autoFocus
											onBlur={(e) => {
												const nextValue =
													param.param_type === "number" || param.param_type === "duration"
														? Number(e.target.value)
														: e.target.value;
												handleEditPreviewParam(name, nextValue);
											}}
											onKeyDown={(e) => {
												if (e.key === 'Enter') {
													const nextValue =
														param.param_type === "number" || param.param_type === "duration"
															? Number(e.target.value)
															: e.target.value;
													handleEditPreviewParam(name, nextValue);
												} else if (e.key === 'Escape') {
													setEditingParam(null);
												}
											}}
											onMouseDown={stopPropagation}
										/>
									) : (
										<span 
											className="param-value" 
											onClick={() => setEditingParam(name)}
											onMouseDown={stopPropagation}
											title="Click to edit"
										>
											{typeof param.value === 'string' ? param.value : JSON.stringify(param.value)}
											<span className="edit-icon" aria-hidden="true">✏️</span>
										</span>
									)}
								</div>
							))}
						</div>
					)}

					{/* Reversibility indicator */}
					{actionPreview.is_reversible && (
						<div className="reversibility-notice">
							<span className="undo-icon" aria-hidden="true">↩️</span>
							{actionPreview.rollback_description || 'This action can be undone'}
						</div>
					)}

					{/* Estimated duration */}
					{actionPreview.estimated_duration_ms && (
						<div className="duration-estimate">
							Est. duration: {actionPreview.estimated_duration_ms}ms
						</div>
					)}

					<div className="preview-actions">
						<button
							type="button"
							className="preview-approve-btn"
							onClick={handleApprovePreview}
							onMouseDown={stopPropagation}
							disabled={actionPreview.state === 'loading'}
						>
							✓ Execute
						</button>
						<button
							type="button"
							className="preview-deny-btn"
							onClick={() => handleDenyPreview('User denied')}
							onMouseDown={stopPropagation}
						>
							✕ Cancel
						</button>
					</div>
				</div>
			)}

			{/* Undo/Redo Controls */}
			{(rollbackStatus.can_undo || rollbackStatus.can_redo) && (
				<div className="undo-redo-controls" role="toolbar" aria-label="Undo/Redo controls">
					<button
						type="button"
						className="undo-btn"
						onClick={handleUndo}
						onMouseDown={stopPropagation}
						disabled={!rollbackStatus.can_undo}
						title={rollbackStatus.undo_description || 'Undo'}
					>
						<span aria-hidden="true">↩️</span> Undo
					</button>
					<button
						type="button"
						className="redo-btn"
						onClick={handleRedo}
						onMouseDown={stopPropagation}
						disabled={!rollbackStatus.can_redo}
						title={rollbackStatus.redo_description || 'Redo'}
					>
						Redo <span aria-hidden="true">↪️</span>
					</button>
					{rollbackStatus.stack_size > 0 && (
						<span className="undo-stack-count" title={`${rollbackStatus.stack_size} actions in history`}>
							({rollbackStatus.stack_size})
						</span>
					)}
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

			{/* Quick Ask - Minimal prompt/response */}
			{gameState.apiKeyConfigured && (
				<div className="quick-ask" role="region" aria-label="Quick ask">
					<div className="quick-ask-header">⚡ Quick Ask</div>
					<form className="quick-ask-form" onSubmit={handleQuickAskSubmit}>
						<input
							type="text"
							className="quick-ask-input"
							value={quickAsk.prompt}
							onChange={handleQuickAskChange}
							placeholder="Ask a quick question..."
							disabled={quickAsk.isLoading}
							onMouseDown={stopPropagation}
							aria-label="Quick ask input"
						/>
						<button
							type="submit"
							className="quick-ask-submit"
							disabled={quickAsk.isLoading}
							onMouseDown={stopPropagation}
						>
							{quickAsk.isLoading ? "⏳" : "→"}
						</button>
					</form>
					{quickAsk.error && (
						<div className="quick-ask-error" role="alert">
							{quickAsk.error}
						</div>
					)}
					{quickAsk.response && (
						<div className="quick-ask-response" role="status">
							{quickAsk.response}
						</div>
					)}
				</div>
			)}

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
									aria-pressed={
										!!autonomySettings?.autoPuzzleFromCompanion
									}
									title={isCompanionMode 
										? "Auto-monitor screen and create puzzles" 
										: "Switch to Companion mode to enable Auto"}
								>
									Auto
								</button>
								<button
									type="button"
									className={`mode-toggle-btn shield ${privacy.settings?.read_only_mode ? "active" : ""}`}
									onMouseDown={stopPropagation}
									onClick={toggleReadOnly}
									aria-pressed={!!privacy.settings?.read_only_mode}
									title="Read-only: Disable all capture and automation"
								>
									🛡️
								</button>
								{/* Autonomy Level Indicator */}
								<span
									className={`autonomy-badge level-${privacy.settings?.autonomy_level || "observer"}`}
									title={`Autonomy: ${
										privacy.settings?.autonomy_level === "suggester" ? "Suggester - confirms actions"
										: privacy.settings?.autonomy_level === "supervised" ? "Supervised - auto-executes safe"
										: privacy.settings?.autonomy_level === "autonomous" ? "Autonomous"
										: "Observer - watch only"
									}`}
								>
									{privacy.settings?.autonomy_level === "suggester" ? "💬"
									: privacy.settings?.autonomy_level === "supervised" ? "⚡"
									: privacy.settings?.autonomy_level === "autonomous" ? "🤖"
									: "👁️"}
								</span>
							</div>
						</div>
						<p className="clue-text">{clueText}</p>
						{gameState.puzzleId && !gameState.hintAvailable && (
							<div className="hint-status">
								<span
									className="hint-charging"
									aria-live="polite"
								>
									<span aria-hidden="true">⏳</span> Hint
									charging...
								</span>
							</div>
						)}
					</div>

					{/* Dialogue Box */}
					{displayDialogue && (
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
									text={displayDialogue}
									speed={TYPEWRITER_SPEED}
								/>
							</div>
							{/* HITL Feedback Buttons */}
							{gameState.state === "idle" && (
								<DialogueFeedback
									content={displayDialogue}
									onFeedback={submitFeedback}
								/>
							)}
						</div>
					)}

					{/* Companion Behavior Suggestion - only show Create Puzzle when Auto is OFF */}
					{companionBehavior && (
						<div className="companion-suggestion" role="status">
							<div className="suggestion-message">
								<span aria-hidden="true">💭</span>{" "}
								{companionBehavior.suggestion}
							</div>
							{companionBehavior.behavior_type === "puzzle" &&
								!autonomySettings?.autoPuzzleFromCompanion && (
								<button
									type="button"
									className="suggestion-action-btn"
									onMouseDown={stopPropagation}
									onClick={withStopPropagation(
										handleGenerateAdaptivePuzzle
									)}
								>
									<span aria-hidden="true">🎯</span> Create
									Puzzle
								</button>
							)}
						</div>
					)}

					{/* Dynamic Action Buttons - Context-aware based on mode and state */}
					{gameState.state === "idle" && (
						<div
							className="action-wrapper"
							role="group"
							aria-label="Actions"
						>
							{/* No active puzzle - show exploration actions */}
							{!gameState.puzzleId && (
								<>
									{/* Game mode OR Companion without Auto: Show manual actions */}
									{(!isCompanionMode || !autonomySettings?.autoPuzzleFromCompanion) && (
										<>
											{/* Analyze Screen - primary action for manual triggering */}
											<button
												type="button"
												className="action-btn primary"
												disabled={!canAnalyze}
												onMouseDown={stopPropagation}
												onClick={withStopPropagation(handleAnalyze)}
												aria-disabled={!canAnalyze}
											>
												<span aria-hidden="true">🔍</span> Analyze Screen
											</button>

											{/* Secondary action based on mode */}
											<button
												type="button"
												className="action-btn secondary"
												onMouseDown={stopPropagation}
												onClick={withStopPropagation(
													isCompanionMode
														? handleGenerateAdaptivePuzzle
														: handleTriggerDynamicPuzzle
												)}
											>
												<span aria-hidden="true">{isCompanionMode ? "🧠" : "🌀"}</span>{" "}
												{isCompanionMode ? "Create Puzzle" : "Start Mystery"}
											</button>
										</>
									)}

									{/* Companion + Auto mode: Show context-aware status */}
									{isCompanionMode && autonomySettings?.autoPuzzleFromCompanion && (
										<div className={`auto-status-indicator ${privacy.settings?.read_only_mode ? "readonly" : !hasFullPrivacyConsent ? "warning" : ""}`}>
											<span className={`auto-pulse ${privacy.settings?.read_only_mode || !hasFullPrivacyConsent ? "paused" : ""}`} aria-hidden="true"></span>
											<span>
												{privacy.settings?.read_only_mode
													? "Read-only mode active"
													: !hasFullPrivacyConsent
														? "Consent required to watch"
														: extensionConnected
															? "Watching your browsing..."
															: "Watching via screenshots..."}
											</span>
										</div>
									)}
								</>
							)}

							{/* Active puzzle - show hint and verify actions */}
							{gameState.puzzleId && (
								<>
									<button
										type="button"
										className={`action-btn ${canHint ? "" : "disabled"}`}
										disabled={!canHint}
										onMouseDown={stopPropagation}
										onClick={withStopPropagation(showHint)}
										aria-disabled={!canHint}
										title={!gameState.hintAvailable ? "Hint is charging..." : "Get a hint"}
									>
										<span aria-hidden="true">{canHint ? "💡" : "⏳"}</span>{" "}
										{canHint ? "Get Hint" : "Hint Charging..."}
									</button>

									<button
										type="button"
										className="action-btn verify-btn"
										onMouseDown={stopPropagation}
										onClick={withStopPropagation(handleVerifyProof)}
									>
										<span aria-hidden="true">📸</span> Prove Finding
									</button>
								</>
							)}
						</div>
					)}

					{/* I'm Stuck Button - HITL Escalation */}
					{gameState.puzzleId && gameState.state === "idle" && (
						<StuckButton
							onStuck={reportStuck}
							puzzleStartTime={puzzleStartTime}
						/>
					)}
				</>
			)}

			{/* Model Capabilities Warning Banner */}
			{modelCapabilities?.warnings?.length > 0 && (
				<div className="capabilities-warning" role="alert">
					<span aria-hidden="true">⚠️</span>
					<div className="capabilities-warning-text">
						{modelCapabilities.warnings.map((w, i) => (
							<div key={i}>{w}</div>
						))}
					</div>
				</div>
			)}

			{/* Token Usage Display */}
			{gameState.apiKeyConfigured && tokenUsage && (
				<div className="token-usage-bar" role="status" aria-label="API usage">
					<span className="token-usage-label">Usage:</span>
					<span className="token-usage-stat gemini" title="Gemini API calls">
						🔮 {tokenUsage.gemini_calls || 0}
					</span>
					<span className="token-usage-stat ollama" title="Ollama calls">
						🦙 {tokenUsage.ollama_calls || 0}
					</span>
					{tokenUsage.estimated_cost_usd > 0 && (
						<span className="token-usage-cost" title="Estimated cost">
							💰 ${tokenUsage.estimated_cost_usd.toFixed(4)}
						</span>
					)}
					<button
						type="button"
						className="token-usage-history-btn"
						onClick={handleShowActionHistory}
						onMouseDown={stopPropagation}
						title="View action history"
					>
						📋
					</button>
					<button
						type="button"
						className="token-usage-history-btn sandbox-btn"
						onClick={handleShowSandboxSettings}
						onMouseDown={stopPropagation}
						title="Sandbox settings"
					>
						🛡️
					</button>
				</div>
			)}

			{/* Action History Modal */}
			{showActionHistory && (
				<div className="ghost-modal-overlay" onMouseDown={closeActionHistory}>
					<div
						className="ghost-modal action-history-modal"
						role="dialog"
						aria-modal="true"
						aria-label="Action history"
						onMouseDown={stopPropagation}
					>
						<div className="ghost-modal-title">
							<span aria-hidden="true">📋</span> Action History
						</div>
						<div className="action-history-list">
							{actionHistory.length === 0 ? (
								<div className="action-history-empty">No actions recorded yet</div>
							) : (
								actionHistory.map((action) => (
									<div
										key={action.id}
										className={`action-history-item status-${action.status}`}
									>
										<div className="action-history-header">
											<span className={`action-status-badge ${action.status}`}>
												{action.status === "approved" || action.status === "executed"
													? "✓"
													: action.status === "denied" || action.status === "failed"
														? "✕"
														: "⏳"}
											</span>
											<span className="action-type">{action.action_type}</span>
											<span className="action-time">
												{new Date((action.created_at || 0) * 1000).toLocaleTimeString()}
											</span>
										</div>
										<div className="action-description">{action.description}</div>
										{action.target && (
											<div className="action-target">Target: {action.target}</div>
										)}
									</div>
								))
							)}
						</div>
						<div className="ghost-modal-actions">
							<button
								type="button"
								className="ghost-modal-btn secondary"
								onMouseDown={stopPropagation}
									onClick={closeActionHistory}
							>
								Close
							</button>
						</div>
					</div>
				</div>
			)}

			{/* Sandbox Settings Modal */}
			{showSandboxSettings && (
				<div className="ghost-modal-overlay" onMouseDown={closeSandboxSettings}>
					<div
						className="ghost-modal sandbox-settings-modal"
						role="dialog"
						aria-modal="true"
						aria-label="Sandbox settings"
						onMouseDown={stopPropagation}
					>
						<div className="ghost-modal-title">
							<span aria-hidden="true">🛡️</span> Sandbox Settings
						</div>
						
						{sandboxSettings && (
							<div className="sandbox-settings-content">
								{/* Trust Level Section */}
								<div className="sandbox-section">
									<div className="sandbox-section-title">Trust Level</div>
									<div className="trust-level-info">
										<span className="trust-score">Score: {sandboxSettings.trust_score}/100</span>
									</div>
									<select
										className="trust-level-select"
										value={sandboxSettings.trust_level}
										onChange={(e) => handleSetTrustLevel(e.target.value)}
									>
										<option value="untrusted">🔒 Untrusted - No file/shell access</option>
										<option value="read_only">📖 Read Only - Read files only</option>
										<option value="limited">⚠️ Limited - Safe operations only</option>
										<option value="elevated">🔓 Elevated - Most operations</option>
										<option value="full">⚡ Full - All operations (dangerous)</option>
									</select>
								</div>

								{/* Shell Categories Section */}
								<div className="sandbox-section">
									<div className="sandbox-section-title">Allowed Shell Commands</div>
									<div className="shell-categories-grid">
										{[
											{ value: "read_info", label: "📊 Read Info", desc: "whoami, date, etc." },
											{ value: "search", label: "🔍 Search", desc: "find, grep, locate" },
											{ value: "package_info", label: "📦 Package Info", desc: "npm list, pip list" },
											{ value: "git_read", label: "📚 Git Read", desc: "git status, log, diff" },
											{ value: "git_write", label: "✍️ Git Write", desc: "git add, commit, push" },
											{ value: "file_manipulation", label: "📁 File Ops", desc: "mkdir, cp, mv" },
											{ value: "file_deletion", label: "🗑️ File Delete", desc: "rm, rmdir" },
											{ value: "network", label: "🌐 Network", desc: "curl, wget, ping" },
											{ value: "process_management", label: "⚙️ Processes", desc: "kill, pkill" },
											{ value: "system_admin", label: "🔧 System Admin", desc: "sudo, chmod" },
											{ value: "arbitrary", label: "💀 Arbitrary", desc: "Any command" },
										].map((cat) => (
											<label key={cat.value} className="shell-category-item">
												<input
													type="checkbox"
													checked={sandboxSettings.allowed_shell_categories?.includes(cat.value) || false}
													onChange={(e) => handleToggleShellCategory(cat.value, e.target.checked)}
												/>
												<span className="category-label">{cat.label}</span>
												<span className="category-desc">{cat.desc}</span>
											</label>
										))}
									</div>
								</div>

								{/* Path Allowlists Section */}
								<div className="sandbox-section">
									<div className="sandbox-section-title">Path Allowlists</div>
									<div className="path-lists">
										<div className="path-list-group">
											<div className="path-list-header">📖 Read Paths</div>
											<div className="path-input-row">
												<input
													className="path-input"
													placeholder="/Users/you/Projects"
													value={readPathInput}
													onChange={(e) => setReadPathInput(e.target.value)}
													onKeyDown={(e) => {
														if (e.key === "Enter") handleAddReadPath();
													}}
												/>
												<button
													type="button"
													className="path-action-btn"
													onClick={handleAddReadPath}
												>
													Add
												</button>
											</div>
											<div className="path-list-items">
												{sandboxSettings.read_allowlist?.length > 0 ? (
													sandboxSettings.read_allowlist.map((path, i) => (
														<div key={i} className="path-item-row">
															<div className="path-item">{path}</div>
															<button
																type="button"
																className="path-remove-btn"
																onClick={() => handleRemoveReadPath(path)}
																aria-label={`Remove read path ${path}`}
															>
																✕
															</button>
														</div>
													))
												) : (
													<div className="path-item empty">No paths configured</div>
												)}
											</div>
										</div>
										<div className="path-list-group">
											<div className="path-list-header">✍️ Write Paths</div>
											<div className="path-input-row">
												<input
													className="path-input"
													placeholder="/Users/you/Projects"
													value={writePathInput}
													onChange={(e) => setWritePathInput(e.target.value)}
													onKeyDown={(e) => {
														if (e.key === "Enter") handleAddWritePath();
													}}
												/>
												<button
													type="button"
													className="path-action-btn"
													onClick={handleAddWritePath}
												>
													Add
												</button>
											</div>
											<div className="path-list-items">
												{sandboxSettings.write_allowlist?.length > 0 ? (
													sandboxSettings.write_allowlist.map((path, i) => (
														<div key={i} className="path-item-row">
															<div className="path-item">{path}</div>
															<button
																type="button"
																className="path-remove-btn"
																onClick={() => handleRemoveWritePath(path)}
																aria-label={`Remove write path ${path}`}
															>
																✕
															</button>
														</div>
													))
												) : (
													<div className="path-item empty">No paths configured</div>
												)}
											</div>
										</div>
									</div>
									{sandboxPathError && (
										<div className="sandbox-error">{sandboxPathError}</div>
									)}
								</div>

								{/* Warning for elevated permissions */}
								{(sandboxSettings.trust_level === "elevated" || sandboxSettings.trust_level === "full") && (
									<div className="sandbox-warning">
										⚠️ <strong>Warning:</strong> Elevated permissions allow file modifications and shell commands.
										The ghost can modify your system. Proceed with caution.
									</div>
								)}
							</div>
						)}

						<div className="ghost-modal-actions">
							<button
								type="button"
								className="ghost-modal-btn secondary"
								onMouseDown={stopPropagation}
								onClick={closeSandboxSettings}
							>
								Close
							</button>
						</div>
					</div>
				</div>
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

					{/* System Controls Accordion */}
					<SystemControlsAccordion
						onExtension={() => openDialog("extension")}
						onPrivacy={openPrivacyModal}
						onChangeKey={() => openDialog("key")}
						onReset={() => openDialog("reset")}
					/>
				</div>
			)}
		</div>
	);
};

export default Ghost;
