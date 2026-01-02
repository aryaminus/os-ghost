import React, { useState, useEffect, useRef } from "react";
import { useGhostGame } from "../hooks/useTauriCommands";

// ASCII Art for the Ghost in different states
const GHOST_SPRITES = {
	idle: `
    .-.
   (o.o)
    |=|
   __|__
  //.=|=.\\
 // .=|=. \\
 \\ .=|=. //
  \\(_=_)//
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
  //.~|~.\\
 // .~|~. \\
 \\ .~|~. //
  \\(_~_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
	searching: `
    .-.
   (>.>)
    |*|
   __|__
  //*=|=*\\
 // *=|=* \\
 \\ *=|=* //
  \\(_*_)//
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
  //!=|=!\\
 // !=|=! \\
 \\ !=|=! //
  \\(_!_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
};

// Typewriter text component
const TypewriterText = ({ text, speed = 30 }) => {
	const [displayed, setDisplayed] = useState("");
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

// Proximity indicator bar
const ProximityBar = ({ proximity }) => {
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

const Ghost = () => {
	const { gameState, setClickable, showHint, captureAndAnalyze } =
		useGhostGame();

	const [isHovered, setIsHovered] = useState(false);

	useEffect(() => {
		setClickable(isHovered);
	}, [isHovered, setClickable]);

	const sprite = GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle;
	const glowIntensity = Math.min(gameState.proximity * 30 + 5, 40);

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

			{/* Puzzle Counter */}
			<div className="puzzle-counter">
				Memory Fragment: {gameState.currentPuzzle + 1}/3
			</div>
		</div>
	);
};

export default Ghost;
