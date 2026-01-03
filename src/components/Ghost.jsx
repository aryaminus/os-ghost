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
		<span className="typewriter-text">
			{displayed}
			<span className="cursor">▌</span>
		</span>
	);
});

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
		<div className="proximity-container">
			<div className="proximity-label">
				<span className="cold-indicator">❄️</span>
				<span className="proximity-text">SIGNAL STRENGTH</span>
				<span className="hot-indicator">🔥</span>
			</div>
			<div className="proximity-bar">
				<div className="proximity-fill" style={fillStyle} />
			</div>
		</div>
	);
});

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

	return (
		<div className="ghost-container" onMouseDown={handleDrag}>
			{/* Ghost Sprite */}
			<div
				className={`ghost-sprite state-${gameState.state}`}
				onClick={handleClick}
				style={spriteStyle}
			>
				<pre className="ascii-art">{sprite}</pre>
			</div>

			{/* Proximity Indicator - Always visible */}
			<ProximityBar proximity={gameState.proximity} />

			{/* Status Issues Section - Show when there are problems */}
			{(!gameState.apiKeyConfigured || !extensionConnected) && (
				<div className="status-section">
					{/* API Key Warning */}
					{!gameState.apiKeyConfigured && (
						<div className="warning-box">
							⚠️ GEMINI_API_KEY not set. AI features disabled.
						</div>
					)}

					{/* Extension Connection Status */}
					{!extensionConnected && (
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
					)}
				</div>
			)}

			{/* Game UI Section - Only show when everything is ready */}
			{gameState.apiKeyConfigured && extensionConnected && (
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
					</div>

					{/* Dialogue Box */}
					{gameState.dialogue && (
						<div
							className={`dialogue-box state-${gameState.state}`}
						>
							<TypewriterText
								text={gameState.dialogue}
								speed={25}
							/>
						</div>
					)}

					{/* Dynamic Puzzle Trigger */}
					{gameState.state === "idle" && !gameState.puzzleId && (
						<button
							className="action-btn"
							onMouseDown={(e) => e.stopPropagation()}
							onClick={(e) => {
								e.stopPropagation();
								triggerDynamicPuzzle();
							}}
						>
							🌀 Investigate This Signal
						</button>
					)}

					{/* Prove Finding Button */}
					{gameState.puzzleId && gameState.state !== "celebrate" && (
						<button
							className="action-btn verify-btn"
							onMouseDown={(e) => e.stopPropagation()}
							onClick={(e) => {
								e.stopPropagation();
								verifyScreenshotProof(); // Call verification
							}}
							style={{
								marginTop: "8px",
								backgroundColor: "var(--accent)",
							}}
						>
							📸 Prove Finding
						</button>
					)}

					{/* Puzzle Counter */}
					<div className="puzzle-counter">
						Memory Fragment: {gameState.currentPuzzle + 1}/∞
					</div>
				</>
			)}

			{/* Dev Tools Panel - Only visible in development mode */}
			{import.meta.env.DEV && gameState.apiKeyConfigured && (
				<div className="dev-tools">
					<div className="dev-tools-header">🛠️ DEV TOOLS</div>
					<div className="dev-tools-buttons">
						<button
							onMouseDown={(e) => e.stopPropagation()}
							onClick={startBackgroundChecks}
							className="dev-btn scan"
						>
							Scan BG
						</button>
						<button
							onMouseDown={(e) => e.stopPropagation()}
							onClick={enableAutonomousMode}
							className="dev-btn auto"
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
