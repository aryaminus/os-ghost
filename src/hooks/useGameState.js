/**
 * @fileoverview Game state management hook.
 * Handles puzzle state, proximity, hints, and ghost animations.
 * @module useGameState
 */

import { invoke } from "@tauri-apps/api/core";
import { useState, useCallback, useRef } from "react";

/** Debug mode - only log in development */
const DEBUG_MODE = import.meta.env.DEV;

function log(...args) {
	if (DEBUG_MODE) console.log("[GameState]", ...args);
}

/**
 * Ghost state type
 * @typedef {"idle" | "thinking" | "searching" | "celebrate" | "observant"} GhostState
 */

/**
 * Game state object
 * @typedef {Object} GameState
 * @property {number} currentPuzzle - Puzzle count/index
 * @property {string} puzzleId - Current puzzle ID
 * @property {string} clue - Current clue text
 * @property {string} hint - Current hint (first hint)
 * @property {string[]} hints - Array of all hints
 * @property {number} hintsRevealed - Number of hints shown
 * @property {number} proximity - Hot/cold proximity (0.0 - 1.0)
 * @property {GhostState} state - Ghost animation state
 * @property {string} dialogue - Current dialogue text
 * @property {string} currentUrl - Last visited URL
 * @property {boolean} apiKeyConfigured - Whether API key is set
 * @property {boolean} hintAvailable - Whether hint timer has elapsed
 */

/** @type {GameState} */
export const initialGameState = {
	currentPuzzle: 0,
	puzzleId: "",
	clue: "",
	hint: "",
	hints: [],
	hintsRevealed: 0,
	proximity: 0,
	state: "idle",
	dialogue: "Waiting for signal... browse the web to begin.",
	currentUrl: "",
	apiKeyConfigured: false,
	hintAvailable: false,
};

/**
 * Hook for managing game state (puzzles, hints, proximity, ghost state).
 * Pure state management - no side effects or event listeners.
 *
 * @returns {Object} Game state and updater functions
 */
export function useGameState() {
	const [gameState, setGameState] = useState(initialGameState);
	const gameStateRef = useRef(gameState);

	// Keep ref in sync
	gameStateRef.current = gameState;

	/**
	 * Update game state partially
	 * @param {Partial<GameState>} updates - State updates
	 */
	const updateGameState = useCallback((updates) => {
		setGameState((prev) => ({ ...prev, ...updates }));
	}, []);

	/**
	 * Set ghost animation state
	 * @param {GhostState} state - New ghost state
	 */
	const setGhostState = useCallback((state) => {
		setGameState((prev) => ({ ...prev, state }));
	}, []);

	/**
	 * Set dialogue text
	 * @param {string} dialogue - Dialogue to display
	 */
	const setDialogue = useCallback((dialogue) => {
		setGameState((prev) => ({ ...prev, dialogue }));
	}, []);

	/**
	 * Update proximity score
	 * @param {number} proximity - New proximity (0.0 - 1.0)
	 */
	const setProximity = useCallback((proximity) => {
		setGameState((prev) => ({ ...prev, proximity }));
	}, []);

	/**
	 * Set current URL
	 * @param {string} url - Current page URL
	 */
	const setCurrentUrl = useCallback((url) => {
		setGameState((prev) => ({ ...prev, currentUrl: url }));
	}, []);

	/**
	 * Load a new puzzle into state
	 * @param {Object} puzzle - Puzzle object from backend
	 */
	const loadPuzzle = useCallback((puzzle) => {
		log("Loading puzzle:", puzzle.id);
		setGameState((prev) => ({
			...prev,
			puzzleId: puzzle.id,
			clue: puzzle.clue,
			hint: puzzle.hint || "",
			hints: puzzle.hints || [],
			hintsRevealed: 0,
			hintAvailable: false,
			state: "thinking",
			dialogue: puzzle.clue,
			currentPuzzle: puzzle.target_description || prev.currentPuzzle,
			proximity: 0,
		}));
	}, []);

	/**
	 * Clear current puzzle (for transition)
	 */
	const clearPuzzle = useCallback(() => {
		setGameState((prev) => ({
			...prev,
			puzzleId: "",
			clue: "",
			hint: "",
			hints: [],
			hintsRevealed: 0,
			proximity: 0,
			state: "idle",
			dialogue: "Fragment restored. The static clears... searching for next signal.",
		}));
	}, []);

	/**
	 * Advance puzzle count and clear current puzzle
	 */
	const advancePuzzle = useCallback(() => {
		log("Advancing to next puzzle");
		setGameState((prev) => ({
			...prev,
			currentPuzzle: prev.currentPuzzle + 1,
			puzzleId: "",
			clue: "",
			hint: "",
			hints: [],
			hintsRevealed: 0,
			proximity: 0,
			state: "idle",
			dialogue: "Fragment restored. The static clears... searching for next signal.",
		}));
	}, []);

	/**
	 * Show next hint
	 */
	const showNextHint = useCallback(async () => {
		const current = gameStateRef.current;
		if (!current.hintAvailable) {
			setDialogue("Patience... the memories need time to surface.");
			return null;
		}

		const hints = current.hints.length > 0 
			? current.hints 
			: current.hint ? [current.hint] : [];

		try {
			const hint = await invoke("get_next_hint", { hints });
			if (hint) {
				setGameState((prev) => ({
					...prev,
					dialogue: hint,
					hintAvailable: false,
					hintsRevealed: prev.hintsRevealed + 1,
				}));
			}
			return hint;
		} catch (err) {
			console.error("[GameState] Failed to get hint:", err);
			setDialogue(current.hint || "No hints available...");
			return null;
		}
	}, [setDialogue]);

	/**
	 * Mark hint as available
	 */
	const enableHint = useCallback(() => {
		setGameState((prev) => ({ ...prev, hintAvailable: true }));
	}, []);

	/**
	 * Set API key configured status
	 * @param {boolean} configured
	 */
	const setApiKeyConfigured = useCallback((configured) => {
		setGameState((prev) => ({ ...prev, apiKeyConfigured: configured }));
	}, []);

	/**
	 * Apply orchestration result to game state
	 * @param {Object} result - OrchestrationResult from backend
	 */
	const applyOrchestrationResult = useCallback((result) => {
		setGameState((prev) => ({
			...prev,
			proximity: result.proximity,
			dialogue: result.message,
			state: result.ghost_state,
		}));
		return result.solved;
	}, []);

	/**
	 * Reset game to initial state
	 * @param {boolean} keepApiKey - Preserve API key configured status
	 */
	const resetGame = useCallback((keepApiKey = true) => {
		setGameState((prev) => ({
			...initialGameState,
			apiKeyConfigured: keepApiKey ? prev.apiKeyConfigured : false,
			dialogue: "Memory wiped. Ready for a new beginning...",
		}));
	}, []);

	/**
	 * Get current state (for use in callbacks that need fresh state)
	 */
	const getState = useCallback(() => gameStateRef.current, []);

	return {
		gameState,
		getState,
		// State updaters
		updateGameState,
		setGhostState,
		setDialogue,
		setProximity,
		setCurrentUrl,
		setApiKeyConfigured,
		// Puzzle management
		loadPuzzle,
		clearPuzzle,
		advancePuzzle,
		// Hints
		showNextHint,
		enableHint,
		// Results
		applyOrchestrationResult,
		// Reset
		resetGame,
	};
}
