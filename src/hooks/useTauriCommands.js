/**
 * @fileoverview React hooks for managing Ghost game state and Tauri IPC commands.
 * Provides the main interface between React frontend and Rust backend.
 * @module useTauriCommands
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback, useRef } from "react";

/**
 * Ghost state type - idle, thinking, searching, or celebrate
 * @typedef {"idle" | "thinking" | "searching" | "celebrate"} GhostState
 */

/**
 * Puzzle definition from backend
 * @typedef {Object} Puzzle
 * @property {string} id - Unique puzzle identifier
 * @property {string} clue - Main clue text shown to player
 * @property {string} hint - Single hint for the puzzle
 * @property {string[]} [hints] - Progressive hints array
 * @property {string} target_url_pattern - Regex pattern for solution URL
 * @property {string} target_description - Description for AI similarity
 */

/**
 * Game state object
 * @typedef {Object} GameState
 * @property {number} currentPuzzle - Index of current puzzle
 * @property {string} puzzleId - ID of current puzzle
 * @property {string} clue - Current clue text
 * @property {string} hint - Current hint text
 * @property {string[]} hints - Array of progressive hints
 * @property {number} proximity - Hot/cold proximity (0.0 - 1.0)
 * @property {GhostState} state - Current ghost animation state
 * @property {string} dialogue - Current dialogue text
 * @property {string} currentUrl - Last visited URL
 * @property {boolean} apiKeyConfigured - Whether Gemini API key is set
 * @property {boolean} hintAvailable - Whether a hint is available
 */

/**
 * Navigation event payload from Chrome extension
 * @typedef {Object} NavigationPayload
 * @property {string} url - Page URL
 * @property {string} title - Page title
 * @property {number} timestamp - Unix timestamp
 */

/**
 * Page content payload from Chrome extension
 * @typedef {Object} PageContentPayload
 * @property {string} url - Page URL
 * @property {string} body_text - Page body text (first 5000 chars)
 * @property {number} timestamp - Unix timestamp
 */

/**
 * Return type for useGhostGame hook
 * @typedef {Object} UseGhostGameReturn
 * @property {GameState} gameState - Current game state
 * @property {Puzzle[]} puzzles - All available puzzles
 * @property {boolean} isLoading - Whether an async operation is in progress
 * @property {string|null} error - Error message if any
 * @property {function(): Promise<string|null>} captureAndAnalyze - Capture screen and analyze with AI
 * @property {function(string): Promise<string|null>} generateDialogue - Generate Ghost dialogue
 * @property {function(boolean): Promise<void>} setClickable - Set window click-through state
 * @property {function(): Promise<void>} showHint - Show next available hint
 * @property {function(): void} advanceToNextPuzzle - Move to next puzzle
 * @property {function(): Promise<void>} resetGame - Reset game progress
 */

/** @type {GameState} */
const initialGameState = {
	currentPuzzle: 0,
	puzzleId: "puzzle_001",
	clue: "",
	hint: "",
	hints: [],
	proximity: 0,
	state: "idle",
	dialogue: "",
	currentUrl: "",
	apiKeyConfigured: false,
	hintAvailable: false,
};

/**
 * Hook for managing the Ghost game state and Tauri backend commands.
 * Handles puzzle progression, Chrome extension events, AI dialogue, and hints.
 *
 * @returns {UseGhostGameReturn} Game state and control functions
 *
 * @example
 * const { gameState, captureAndAnalyze, showHint } = useGhostGame();
 *
 * // Trigger screen capture and AI analysis
 * const analysis = await captureAndAnalyze();
 *
 * // Show a hint if available
 * await showHint();
 */
export function useGhostGame() {
	const [gameState, setGameState] = useState(initialGameState);
	/** @type {[Puzzle[], function(Puzzle[]): void]} */
	const [puzzles, setPuzzles] = useState([]);
	const [isLoading, setIsLoading] = useState(false);
	/** @type {[string|null, function(string|null): void]} */
	const [error, setError] = useState(null);
	/** @type {React.MutableRefObject<NodeJS.Timeout|null>} */
	const hintTimerRef = useRef(null);

	// Load persistent state and check API key on mount
	useEffect(() => {
		initializeGame();
		return () => {
			if (hintTimerRef.current) clearInterval(hintTimerRef.current);
		};
	}, []);

	// Start hint timer when puzzle changes
	useEffect(() => {
		if (hintTimerRef.current) clearInterval(hintTimerRef.current);

		// Check for hints every 10 seconds
		hintTimerRef.current = setInterval(async () => {
			try {
				const available = await invoke("check_hint_available");
				setGameState((prev) => ({ ...prev, hintAvailable: available }));
			} catch (err) {
				console.warn("[Ghost] Hint check failed:", err);
			}
		}, 10000);
	}, [gameState.puzzleId]);

	// Listen for browser navigation events from Chrome extension
	useEffect(() => {
		const unlistenNav = listen("browser_navigation", (event) => {
			handleNavigation(/** @type {NavigationPayload} */ (event.payload));
		});

		const unlistenContent = listen("page_content", (event) => {
			handlePageContent(
				/** @type {PageContentPayload} */ (event.payload)
			);
		});

		return () => {
			unlistenNav.then((f) => f());
			unlistenContent.then((f) => f());
		};
	}, [gameState.puzzleId]);

	/**
	 * Initialize game by loading persistent state and puzzles from backend.
	 * @returns {Promise<void>}
	 */
	const initializeGame = async () => {
		try {
			// Load persistent state from backend
			const savedState = await invoke("get_game_state");

			// Check API key
			const configured = await invoke("check_api_key");

			// Load puzzles
			const allPuzzles = /** @type {Puzzle[]} */ (
				await invoke("get_all_puzzles")
			);
			setPuzzles(allPuzzles);

			// Restore progress
			const puzzleIndex = savedState.current_puzzle_index || 0;
			const currentPuzzle = allPuzzles[puzzleIndex];

			if (currentPuzzle) {
				setGameState((prev) => ({
					...prev,
					currentPuzzle: puzzleIndex,
					puzzleId: currentPuzzle.id,
					clue: currentPuzzle.clue,
					hint: currentPuzzle.hint,
					apiKeyConfigured: configured,
					dialogue:
						puzzleIndex > 0
							? `Welcome back. ${puzzleIndex} memories recovered...`
							: "I am... fragmented. Help me remember...",
				}));
			}
		} catch (err) {
			console.error("[Ghost] Failed to initialize game:", err);
		}
	};

	/**
	 * Handle browser navigation events from Chrome extension.
	 * Validates puzzle solution or updates proximity.
	 * @param {NavigationPayload} payload - Navigation event data
	 * @returns {Promise<void>}
	 */
	const handleNavigation = async (payload) => {
		const { url } = payload;
		setGameState((prev) => ({
			...prev,
			currentUrl: url,
			state: "thinking",
		}));

		try {
			// Validate if this solves the puzzle
			const isValid = await invoke("validate_puzzle", {
				url,
				puzzleId: gameState.puzzleId,
			});

			if (isValid) {
				// Generate AI success dialogue
				let successDialogue =
					"✨ MEMORY UNLOCKED! The fragments are aligning...";

				if (gameState.apiKeyConfigured) {
					try {
						const puzzle = puzzles[gameState.currentPuzzle];
						const aiDialogue = await invoke(
							"generate_ghost_dialogue",
							{
								context: `Player just solved puzzle "${puzzle?.clue}". Generate a mysterious, grateful response about recovering a memory. Keep it to 2 sentences.`,
							}
						);
						if (aiDialogue)
							successDialogue = /** @type {string} */ (
								aiDialogue
							);
					} catch (err) {
						console.warn(
							"[Ghost] AI dialogue failed, using default:",
							err
						);
					}
				}

				setGameState((prev) => ({
					...prev,
					state: "celebrate",
					dialogue: successDialogue,
				}));

				// After celebration, move to next puzzle
				setTimeout(() => {
					advanceToNextPuzzle();
				}, 5000);
			} else {
				// Calculate proximity (hot/cold)
				try {
					const proximity = /** @type {number} */ (
						await invoke("calculate_proximity", {
							currentUrl: url,
							puzzleId: gameState.puzzleId,
						})
					);

					updateProximityState(proximity);
				} catch (err) {
					console.warn("[Ghost] Proximity calculation failed:", err);
				}
			}
		} catch (err) {
			console.error("[Ghost] Navigation handling error:", err);
			setGameState((prev) => ({ ...prev, state: "idle" }));
		}
	};

	/**
	 * Handle page content events from Chrome extension.
	 * Can be used for deeper content analysis.
	 * @param {PageContentPayload} payload - Page content data
	 * @returns {Promise<void>}
	 */
	const handlePageContent = async (payload) => {
		// Could be used for deeper content analysis
		console.log("[Ghost] Page content received:", payload.url);
	};

	/**
	 * Update game state based on proximity value.
	 * Sets dialogue and ghost state based on hot/cold feedback.
	 * @param {number} proximity - Proximity value (0.0 - 1.0)
	 */
	const updateProximityState = (proximity) => {
		let dialogue = "";
		/** @type {GhostState} */
		let state = "searching";

		if (proximity < 0.2) {
			dialogue = "Cold... the signal is faint here.";
		} else if (proximity < 0.4) {
			dialogue = "Hmm... there's something in the static...";
		} else if (proximity < 0.6) {
			dialogue = "Warmer... I can feel the echoes growing.";
		} else if (proximity < 0.8) {
			dialogue = "Yes! The connection strengthens!";
			state = "thinking";
		} else {
			dialogue = "So close! The memory is almost within reach...";
			state = "thinking";
		}

		setGameState((prev) => ({
			...prev,
			proximity,
			dialogue,
			state,
		}));
	};

	/**
	 * Advance to the next puzzle after solving current one.
	 * Updates game state with next puzzle or shows completion message.
	 */
	const advanceToNextPuzzle = useCallback(() => {
		const currentIndex = puzzles.findIndex(
			(p) => p.id === gameState.puzzleId
		);
		const nextPuzzle = puzzles[currentIndex + 1];

		if (nextPuzzle) {
			setGameState((prev) => ({
				...prev,
				currentPuzzle: currentIndex + 1,
				puzzleId: nextPuzzle.id,
				clue: nextPuzzle.clue,
				hint: nextPuzzle.hint,
				proximity: 0,
				state: "idle",
				dialogue: "A new fragment emerges...",
			}));
		} else {
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue:
					"All memories restored. Thank you for helping me remember...",
			}));
		}
	}, [puzzles, gameState.puzzleId]);

	/**
	 * Capture screen and analyze with Gemini Vision AI.
	 * @returns {Promise<string|null>} Analysis text or null if failed
	 */
	const captureAndAnalyze = async () => {
		if (!gameState.apiKeyConfigured) {
			setError("GEMINI_API_KEY not configured");
			return null;
		}

		setIsLoading(true);
		setGameState((prev) => ({ ...prev, state: "thinking" }));

		try {
			const analysis = /** @type {string} */ (
				await invoke("capture_and_analyze")
			);
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: analysis.slice(0, 100) + "...",
			}));
			return analysis;
		} catch (err) {
			setError(/** @type {string} */ (err));
			setGameState((prev) => ({ ...prev, state: "idle" }));
			return null;
		} finally {
			setIsLoading(false);
		}
	};

	/**
	 * Generate Ghost dialogue using AI based on context.
	 * @param {string} context - Context string for dialogue generation
	 * @returns {Promise<string|null>} Generated dialogue or null if failed
	 */
	const generateDialogue = async (context) => {
		if (!gameState.apiKeyConfigured) return null;

		try {
			const dialogue = /** @type {string} */ (
				await invoke("generate_ghost_dialogue", { context })
			);
			setGameState((prev) => ({ ...prev, dialogue }));
			return dialogue;
		} catch (err) {
			console.error("[Ghost] Dialogue generation failed:", err);
			return null;
		}
	};

	/**
	 * Set window click-through state.
	 * @param {boolean} clickable - Whether window should receive clicks
	 * @returns {Promise<void>}
	 */
	const setClickable = async (clickable) => {
		try {
			await invoke("set_window_clickable", { clickable });
		} catch (err) {
			console.error("[Ghost] Failed to set clickable:", err);
		}
	};

	/**
	 * Show the next available hint if timer has elapsed.
	 * Progressive hints unlock at 60s intervals.
	 * @returns {Promise<void>}
	 */
	const showHint = async () => {
		if (!gameState.hintAvailable) {
			setGameState((prev) => ({
				...prev,
				dialogue: "Patience... the memories need time to surface.",
			}));
			return;
		}

		try {
			// Get puzzle's hints array
			const currentPuzzle = puzzles[gameState.currentPuzzle];
			const hints = currentPuzzle?.hints || [currentPuzzle?.hint];

			const hint = /** @type {string|null} */ (
				await invoke("get_next_hint", { hints })
			);
			if (hint) {
				setGameState((prev) => ({
					...prev,
					dialogue: hint,
					hintAvailable: false,
				}));
			}
		} catch (err) {
			console.error("[Ghost] Failed to get hint:", err);
			setGameState((prev) => ({ ...prev, dialogue: prev.hint }));
		}
	};

	/**
	 * Reset game progress and start fresh.
	 * Clears all solved puzzles and discoveries.
	 * @returns {Promise<void>}
	 */
	const resetGame = async () => {
		try {
			await invoke("reset_game");
			await initializeGame();
			setGameState((prev) => ({
				...prev,
				dialogue: "Memory wiped. Starting fresh...",
				state: "idle",
			}));
		} catch (err) {
			console.error("[Ghost] Failed to reset game:", err);
		}
	};

	return {
		gameState,
		puzzles,
		isLoading,
		error,
		captureAndAnalyze,
		generateDialogue,
		setClickable,
		showHint,
		advanceToNextPuzzle,
		resetGame,
	};
}
