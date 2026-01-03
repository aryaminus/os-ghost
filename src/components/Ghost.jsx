/**
 * @fileoverview Ghost character component with ASCII art sprite, animations,
 * proximity indicator, dialogue box, and game UI.
 * @module Ghost
 */

import React, { useState, useEffect, useRef } from "react";
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
const TypewriterText = ({ text, speed = 30 }) => {
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
};

/**
 * Proximity indicator bar showing hot/cold feedback.
 * Changes color based on proximity value.
 *
 * @param {Object} props - Component props
 * @param {number} props.proximity - Proximity value (0.0 - 1.0)
 * @returns {JSX.Element} Proximity bar element
 */
const ProximityBar = ({ proximity }) => {
	/**
	 * Get color based on proximity value.
	 * @returns {string} CSS color value
	 */
	const getColor = () => {
		if (proximity < 0.3) return "var(--cold)";
		if (proximity < 0.6) return "var(--warm)";
		return "var(--hot)";
	};

	return (
		<div className="proximity-container">
			<div className="proximity-label">
				<span className="cold-indicator">❄️</span>
				<span className="proximity-text">SIGNAL STRENGTH</span>
				<span className="hot-indicator">🔥</span>
			</div>
			<div className="proximity-bar">
				<div
					className="proximity-fill"
					style={{
						width: `${proximity * 100}%`,
						backgroundColor: getColor(),
					}}
				/>
			</div>
		</div>
	);
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
		puzzles,
		setClickable,
		showHint,
		captureAndAnalyze,
		triggerDynamicPuzzle,
	} = useGhostGame();

	// Expose triggerDynamicPuzzle for the button handler
	useEffect(() => {
		window.__triggerDynamic = triggerDynamicPuzzle;
		return () => {
			delete window.__triggerDynamic;
		};
	}, [triggerDynamicPuzzle]);

	const [isHovered, setIsHovered] = useState(false);

	// Update window click-through based on hover state
	useEffect(() => {
		setClickable(isHovered);
	}, [isHovered, setClickable]);

	const sprite = GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle;
	const glowIntensity = Math.min(gameState.proximity * 30 + 5, 40);

	/**
	 * Handle click on Ghost.
	 * Captures screen if idle, shows hint otherwise.
	 */
	const handleClick = () => {
		if (gameState.state === "idle") {
			captureAndAnalyze();
		} else {
			showHint();
		}
	};

	return (
		<div
			className="ghost-container"
			onMouseEnter={() => setIsHovered(true)}
			onMouseLeave={() => setIsHovered(false)}
		>
			{/* Ghost Sprite */}
			<div
				className={`ghost-sprite state-${gameState.state}`}
				onClick={handleClick}
				style={{
					"--glow-intensity": `${glowIntensity}px`,
					"--glow-color":
						gameState.state === "celebrate"
							? "var(--celebrate-glow)"
							: "var(--ghost-glow)",
				}}
			>
				<pre className="ascii-art">{sprite}</pre>
			</div>

			{/* Proximity Indicator */}
			<ProximityBar proximity={gameState.proximity} />

			{/* Current Clue */}
			<div className="clue-box">
				<div className="clue-header">📜 CURRENT MYSTERY</div>
				<p className="clue-text">
					{gameState.clue || "Loading puzzle..."}
				</p>
			</div>

			{/* Dialogue Box */}
			{gameState.dialogue && (
				<div className={`dialogue-box state-${gameState.state}`}>
					<TypewriterText text={gameState.dialogue} speed={25} />
				</div>
			)}

			{/* API Key Warning */}
			{!gameState.apiKeyConfigured && (
				<div className="warning-box">
					⚠️ GEMINI_API_KEY not set. AI features disabled.
				</div>
			)}

			{/* Dynamic Puzzle Trigger */}
			{gameState.apiKeyConfigured && gameState.state === "idle" && (
				<button
					className="dynamic-trigger-btn"
					onClick={(e) => {
						e.stopPropagation();
						// Import dynamically if needed or passed from hook
						if (window.__triggerDynamic) window.__triggerDynamic();
					}}
					style={{
						marginTop: "10px",
						background: "rgba(0, 255, 255, 0.1)",
						border: "1px solid var(--ghost-glow)",
						color: "var(--ghost-glow)",
						padding: "5px 10px",
						cursor: "pointer",
						fontSize: "0.8em",
						borderRadius: "4px",
					}}
				>
					🌀 Investigate This Signal
				</button>
			)}

			{/* Puzzle Counter */}
			<div className="puzzle-counter">
				Memory Fragment: {gameState.currentPuzzle + 1}/{puzzles.length || 3}
			</div>
		</div>
	);
};

export default Ghost;
