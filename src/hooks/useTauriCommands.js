import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback } from "react";

/**
 * Game state structure
 */
const initialGameState = {
	currentPuzzle: 0,
	puzzleId: "puzzle_001",
	clue: "",
	hint: "",
	proximity: 0,
	state: "idle", // idle, thinking, searching, celebrate
	dialogue: "",
	currentUrl: "",
	apiKeyConfigured: false,
};

/**
 * Hook for managing the Ghost game state and Tauri commands
 */
export function useGhostGame() {
	const [gameState, setGameState] = useState(initialGameState);
	const [puzzles, setPuzzles] = useState([]);
	const [isLoading, setIsLoading] = useState(false);
	const [error, setError] = useState(null);

	// Check API key on mount
	useEffect(() => {
		checkApiKey();
		loadPuzzles();
	}, []);

	// Listen for browser navigation events from Chrome extension
	useEffect(() => {
		const unlistenNav = listen("browser_navigation", (event) => {
			handleNavigation(event.payload);
		});

		const unlistenContent = listen("page_content", (event) => {
			handlePageContent(event.payload);
		});

		return () => {
			unlistenNav.then((f) => f());
			unlistenContent.then((f) => f());
		};
	}, [gameState.puzzleId]);

	const checkApiKey = async () => {
		try {
			const configured = await invoke("check_api_key");
			setGameState((prev) => ({ ...prev, apiKeyConfigured: configured }));
		} catch (err) {
			console.error("Failed to check API key:", err);
		}
	};

	const loadPuzzles = async () => {
		try {
			const allPuzzles = await invoke("get_all_puzzles");
			setPuzzles(allPuzzles);
			if (allPuzzles.length > 0) {
				setGameState((prev) => ({
					...prev,
					puzzleId: allPuzzles[0].id,
					clue: allPuzzles[0].clue,
					hint: allPuzzles[0].hint,
				}));
			}
		} catch (err) {
			console.error("Failed to load puzzles:", err);
		}
	};

	const handleNavigation = async (payload) => {
		const { url, title } = payload;
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
				setGameState((prev) => ({
					...prev,
					state: "celebrate",
					dialogue:
						"✨ MEMORY UNLOCKED! The fragments are aligning...",
				}));

				// After celebration, move to next puzzle
				setTimeout(() => {
					advanceToNextPuzzle();
				}, 5000);
			} else {
				// Calculate proximity (hot/cold)
				try {
					const proximity = await invoke("calculate_proximity", {
						currentUrl: url,
						puzzleId: gameState.puzzleId,
					});

					updateProximityState(proximity);
				} catch (err) {
					console.warn("Proximity calculation failed:", err);
				}
			}
		} catch (err) {
			console.error("Navigation handling error:", err);
			setGameState((prev) => ({ ...prev, state: "idle" }));
		}
	};

	const handlePageContent = async (payload) => {
		// Could be used for deeper content analysis
		console.log("Page content received:", payload.url);
	};

	const updateProximityState = (proximity) => {
		let dialogue = "";
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

	const captureAndAnalyze = async () => {
		if (!gameState.apiKeyConfigured) {
			setError("GEMINI_API_KEY not configured");
			return null;
		}

		setIsLoading(true);
		setGameState((prev) => ({ ...prev, state: "thinking" }));

		try {
			const analysis = await invoke("capture_and_analyze");
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: analysis.slice(0, 100) + "...",
			}));
			return analysis;
		} catch (err) {
			setError(err);
			setGameState((prev) => ({ ...prev, state: "idle" }));
			return null;
		} finally {
			setIsLoading(false);
		}
	};

	const generateDialogue = async (context) => {
		if (!gameState.apiKeyConfigured) return null;

		try {
			const dialogue = await invoke("generate_ghost_dialogue", {
				context,
			});
			setGameState((prev) => ({ ...prev, dialogue }));
			return dialogue;
		} catch (err) {
			console.error("Dialogue generation failed:", err);
			return null;
		}
	};

	const setClickable = async (clickable) => {
		try {
			await invoke("set_window_clickable", { clickable });
		} catch (err) {
			console.error("Failed to set clickable:", err);
		}
	};

	const showHint = () => {
		setGameState((prev) => ({ ...prev, dialogue: prev.hint }));
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
	};
}
