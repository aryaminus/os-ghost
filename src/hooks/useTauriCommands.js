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
 * Result from backend agent orchestration
 * @typedef {Object} OrchestrationResult
 * @property {string} message - Combined dialogue/message
 * @property {number} proximity - Proximity score (0.0 - 1.0)
 * @property {boolean} solved - Whether puzzle was solved
 * @property {number|null} show_hint - Suggested hint index (if any)
 * @property {string} ghost_state - Ghost state (idle, thinking, searching, celebrate)
 * @property {Object[]} agent_outputs - Debug outputs from agents
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
	puzzleId: "", // Start empty, wait for dynamic generation
	clue: "",
	hint: "",
	hints: [],
	proximity: 0,
	state: "idle",
	dialogue: "Waiting for signal... browse the web to begin.",
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
// Global cache to persist content across hook-remounts
let globalLatestContent = null;

export function useGhostGame() {
	const [gameState, setGameState] = useState(initialGameState);
	/** @type {[Puzzle[], function(Puzzle[]): void]} */
	const [puzzles, setPuzzles] = useState([]);
	const [isLoading, setIsLoading] = useState(false);
	/** @type {[string|null, function(string|null): void]} */
	const [error, setError] = useState(null);
	const [extensionConnected, setExtensionConnected] = useState(false);
	/** @type {React.MutableRefObject<NodeJS.Timeout|null>} */
	const hintTimerRef = useRef(null);
	/** @type {React.MutableRefObject<PageContentPayload|null>} */
	const latestContentRef = useRef(globalLatestContent);
	/** @type {React.MutableRefObject<GameState>} */
	const gameStateRef = useRef(gameState);

	// Keep ref in sync with state
	useEffect(() => {
		gameStateRef.current = gameState;
	}, [gameState]);

	/**
	 * Handle page content events from Chrome extension.
	 * Can be used for deeper content analysis.
	 * @param {PageContentPayload} payload - Page content data
	 * @returns {Promise<void>}
	 */

	const handlePageContent = useCallback(async (payload) => {
		console.log("[Ghost] handlePageContent received:", payload.url);
		globalLatestContent = payload;
		latestContentRef.current = payload;

		// If no puzzle is active and API key is configured, generate one!
		const currentState = gameStateRef.current;
		if (!currentState.puzzleId && currentState.apiKeyConfigured) {
			console.log(
				"[Ghost] No active puzzle, triggering generation from content..."
			);
			// call triggerDynamicPuzzle but we can't call it directly if it's defined below
			// Use the function directly or move definitions.
			// To avoid hoisting issues, we'll access it via a ref or just call generateDynamicPuzzle directly
			// since we have the payload right here.
			const title = document.title || "Current Page";
			// We need to access generateDynamicPuzzle which is defined below.
			// Hooks order matters. Let's use a ref or useEffect...
			// Actually, best to just trigger an effect or use a mutable ref for the trigger function.
			triggerPuzzleGenerationRef.current &&
				triggerPuzzleGenerationRef.current();
		}
	}, []);

	/**
	 * Wrapper to generate puzzle from current context
	 */
	const triggerDynamicPuzzle = useCallback(async () => {
		console.log(
			"[Ghost] Triggering dynamic puzzle. Content ref:",
			latestContentRef.current
		);

		// Simple retry mechanism (waits up to 2 seconds)
		let attempts = 0;
		while (!latestContentRef.current && attempts < 10) {
			console.log(
				`[Ghost] Content missing, waiting... (${attempts + 1}/10)`
			);
			await new Promise((r) => setTimeout(r, 200));
			attempts++;
		}

		if (latestContentRef.current) {
			const { url, body_text } = latestContentRef.current;
			// PageContentPayload only has url/body_text, use document title or fallback
			const title = document.title || "Current Page";

			return await generateDynamicPuzzle(url, title, body_text || "");
		} else {
			console.warn(
				"[Ghost] No content available for generation. Ref is null/undefined."
			);
			return null;
		}
	}, [gameState.apiKeyConfigured]); // eslint-disable-line react-hooks/exhaustive-deps
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

	// Store refs for callbacks to avoid re-subscribing on every render
	const handleNavigationRef = useRef(null);
	const advanceToNextPuzzleRef = useRef(null);
	const triggerPuzzleGenerationRef = useRef(null);

	// Update ref when function changes
	useEffect(() => {
		triggerPuzzleGenerationRef.current = triggerDynamicPuzzle;
	}, [triggerDynamicPuzzle]);

	// Listen for browser navigation events from Chrome extension
	// Only set up once on mount to avoid re-subscribing
	useEffect(() => {
		// Store unlisten functions
		let unlistenNav = null;
		let unlistenContent = null;
		let unlistenAutonomous = null;
		let unlistenConnected = null;
		let unlistenDisconnected = null;

		console.log("[Ghost] Setting up Tauri event listeners...");

		const setupListeners = async () => {
			unlistenNav = await listen("browser_navigation", (event) => {
				console.log(
					"[Ghost] Received browser_navigation event:",
					event.payload
				);
				handleNavigationRef.current?.(
					/** @type {NavigationPayload} */ (event.payload)
				);
			});

			unlistenContent = await listen("page_content", (event) => {
				console.log(
					"[Ghost] Received page_content event:",
					event.payload
				);
				handlePageContent(
					/** @type {PageContentPayload} */ (event.payload)
				);
			});

			console.log("[Ghost] Event listeners registered successfully");

			// Test if events work by emitting a test event from backend
			try {
				const testResult = await invoke("check_api_key");
				console.log(
					"[Ghost] Backend connection verified, API key configured:",
					testResult
				);
			} catch (err) {
				console.error("[Ghost] Backend connection test failed:", err);
			}

			// Listen for autonomous mode progress events
			// Listen for extension connection events
			unlistenConnected = await listen("extension_connected", () => {
				console.log("[Ghost] Extension connected");
				setExtensionConnected(true);
			});

			unlistenDisconnected = await listen(
				"extension_disconnected",
				() => {
					console.log("[Ghost] Extension disconnected");
					setExtensionConnected(false);
				}
			);

			unlistenAutonomous = await listen(
				"autonomous_progress",
				(event) => {
					const { proximity, message, solved, finished } =
						event.payload;
					setGameState((prev) => ({
						...prev,
						proximity,
						dialogue: message,
						state: solved
							? "celebrate"
							: finished
								? "idle"
								: "searching",
					}));

					if (solved) {
						setTimeout(
							() => advanceToNextPuzzleRef.current(),
							5000
						);
					}
				}
			);
		};

		setupListeners();

		return () => {
			if (unlistenNav) unlistenNav();
			if (unlistenContent) unlistenContent();
			if (unlistenAutonomous) unlistenAutonomous();
			if (unlistenConnected) unlistenConnected();
			if (unlistenDisconnected) unlistenDisconnected();
		};
	}, [handlePageContent]); // Only handlePageContent is stable (empty deps useCallback)

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

			setGameState((prev) => ({
				...prev,
				apiKeyConfigured: configured,
				dialogue:
					"Connection established. I need to see what you see... browse to a page.",
				// Ensure we don't start with a default puzzle
				puzzleId: "",
				clue: "",
			}));

			// If we already have content (re-mount), trigger generation
			if (latestContentRef.current && configured) {
				console.log(
					"[Ghost] Found existing content, triggering initial puzzle..."
				);
				triggerDynamicPuzzle();
			}
		} catch (err) {
			console.error("[Ghost] Failed to initialize game:", err);
		}
	};

	/**
	 * Advance to the next puzzle after solving current one.
	 * Updates game state with next puzzle or shows completion message.
	 */
	const advanceToNextPuzzle = useCallback(() => {
		console.log("[Ghost] Puzzle solved! Preparing next mystery...");

		// For dynamic mode, we clear the current puzzle and request a new one
		// This creates an infinite loop of procedural puzzles
		setGameState((prev) => ({
			...prev,
			currentPuzzle: prev.currentPuzzle + 1,
			puzzleId: "", // Clear ID to trigger "waiting" state
			clue: "",
			hint: "",
			proximity: 0,
			state: "idle",
			dialogue:
				"Fragment restored. The static clears... searching for next signal.",
		}));

		// Use the ref to trigger generation immediately if we have content
		// This makes it feel seamless if the user is still on a page
		if (latestContentRef.current && triggerPuzzleGenerationRef.current) {
			setTimeout(() => {
				console.log(
					"[Ghost] Auto-triggering next puzzle generation..."
				);
				triggerPuzzleGenerationRef.current();
			}, 2000); // Small delay for effect
		}
	}, []);

	/**
	 * Handle browser navigation events from Chrome extension.
	 * Delegates to backend Agent Orchestrator for analysis and game state updates.
	 * Uses ref to avoid stale closures in event listeners.
	 * @param {NavigationPayload} payload - Navigation event data
	 * @returns {Promise<void>}
	 */
	const handleNavigation = useCallback(
		async (payload) => {
			const { url, title } = payload;
			const currentState = gameStateRef.current;

			setGameState((prev) => ({
				...prev,
				currentUrl: url,
				state: "thinking",
			}));

			// Skip if API key not configured
			if (!currentState.apiKeyConfigured) {
				setGameState((prev) => ({ ...prev, state: "idle" }));
				return;
			}

			// Skip if no puzzle is active (waiting for content)
			if (!currentState.puzzleId) {
				// We need content to generate a puzzle.
				// We rely on handlePageContent to trigger generation.
				console.log("[Ghost] No active puzzle, waiting for content...");
				return;
			}

			try {
				// Call multi-agent orchestrator
				const result = /** @type {OrchestrationResult} */ (
					await invoke("process_agent_cycle", {
						context: {
							url,
							title,
							content: latestContentRef.current?.body_text || "",
							puzzle_id: currentState.puzzleId,
							puzzle_clue: currentState.clue,
							target_pattern: "", // Backend handles this from puzzle ID
							hints: currentState.hints,
							hints_revealed: 0, // Should be tracked in session state
						},
					})
				);

				if (result) {
					setGameState((prev) => ({
						...prev,
						proximity: result.proximity,
						dialogue: result.message,
						state: result.ghost_state,
					}));

					// Handle solved state
					if (result.solved) {
						setTimeout(() => {
							advanceToNextPuzzle();
						}, 5000);
					}
				}
			} catch (err) {
				console.error("[Ghost] Agent cycle failed:", err);
				setGameState((prev) => ({ ...prev, state: "idle" }));
			}
		},
		[advanceToNextPuzzle]
	);

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
	const setClickable = useCallback(async (clickable) => {
		try {
			await invoke("set_window_clickable", { clickable });
		} catch (err) {
			console.error("[Ghost] Failed to set clickable:", err);
		}
	}, []);

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

	/**
	 * Generate a dynamic puzzle based on current page content.
	 * Creates unique contextual puzzles from what the user is viewing.
	 * @param {string} url - Current page URL
	 * @param {string} title - Current page title
	 * @param {string} content - Page body text (first 500 chars)
	 * @returns {Promise<Object|null>} Generated puzzle or null if failed
	 */
	const generateDynamicPuzzle = async (url, title, content) => {
		if (!gameState.apiKeyConfigured) {
			console.warn("[Ghost] Cannot generate dynamic puzzle - no API key");
			return null;
		}

		try {
			setGameState((prev) => ({
				...prev,
				state: "thinking",
				dialogue: "Crafting a new mystery from the digital ether...",
			}));

			const puzzle = await invoke("generate_dynamic_puzzle", {
				url,
				title,
				content: content.slice(0, 500),
			});

			if (puzzle) {
				console.log("[Ghost] Generated dynamic puzzle:", puzzle.id);
				setGameState((prev) => ({
					...prev,
					puzzleId: puzzle.id, // Use ID from backend (registered puzzle)
					clue: puzzle.clue,
					hint: puzzle.hint || puzzle.hints?.[0] || "",
					hints: puzzle.hints || [],
					state: "idle",
					dialogue:
						"A new fragment materializes from your journey...",
				}));

				return puzzle;
			}
		} catch (err) {
			console.error("[Ghost] Failed to generate dynamic puzzle:", err);
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: "The static obscures this memory...",
			}));
		}

		return null;
	};

	// Keep refs in sync with functions
	useEffect(() => {
		handleNavigationRef.current = handleNavigation;
	}, [handleNavigation]);

	useEffect(() => {
		advanceToNextPuzzleRef.current = advanceToNextPuzzle;
	}, [advanceToNextPuzzle]);

	/**
	 * Trigger background analysis using Parallel Workflow.
	 * @returns {Promise<void>}
	 */
	const startBackgroundChecks = useCallback(async () => {
		const currentState = gameStateRef.current;
		console.log("[Ghost] startBackgroundChecks called", {
			hasContent: !!latestContentRef.current,
			apiKeyConfigured: currentState.apiKeyConfigured,
		});

		if (!currentState.apiKeyConfigured) {
			console.warn(
				"[Ghost] Cannot run background checks: API key not configured"
			);
			return;
		}

		if (!latestContentRef.current) {
			console.warn(
				"[Ghost] Cannot run background checks: No page content from extension. Browse a page first."
			);
			setGameState((prev) => ({
				...prev,
				dialogue: "No page content detected. Browse to a page first...",
			}));
			return;
		}

		if (!currentState.puzzleId) {
			setGameState((prev) => ({
				...prev,
				dialogue:
					"No mystery to investigate yet... waiting for signal.",
			}));
			return;
		}

		const { url, body_text } = latestContentRef.current;
		try {
			setGameState((prev) => ({
				...prev,
				state: "searching",
				dialogue: "Scanning the digital ether...",
			}));
			await invoke("start_background_checks", {
				context: {
					url,
					title: document.title || "Current Page",
					content: body_text || "",
					puzzle_id: currentState.puzzleId,
					puzzle_clue: currentState.clue,
					target_pattern: "",
					hints: currentState.hints || [],
					hints_revealed: 0,
				},
			});
			console.log("[Ghost] Background checks completed");
		} catch (err) {
			console.error("[Ghost] Background checks failed:", err);
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: "Background scan encountered interference...",
			}));
		}
	}, []);

	/**
	 * Enable autonomous mode (Loop Workflow).
	 * @returns {Promise<void>}
	 */
	const enableAutonomousMode = useCallback(async () => {
		const currentState = gameStateRef.current;
		console.log("[Ghost] enableAutonomousMode called", {
			hasContent: !!latestContentRef.current,
			apiKeyConfigured: currentState.apiKeyConfigured,
		});

		if (!currentState.apiKeyConfigured) {
			console.warn(
				"[Ghost] Cannot enable autonomous mode: API key not configured"
			);
			return;
		}

		if (!latestContentRef.current) {
			console.warn(
				"[Ghost] Cannot enable autonomous mode: No page content from extension. Browse a page first."
			);
			setGameState((prev) => ({
				...prev,
				dialogue: "No page content detected. Browse to a page first...",
			}));
			return;
		}

		if (!currentState.puzzleId) {
			setGameState((prev) => ({
				...prev,
				dialogue: "I cannot work autonomously without a target...",
			}));
			return;
		}

		const { url, body_text } = latestContentRef.current;

		try {
			setGameState((prev) => ({
				...prev,
				state: "thinking",
				dialogue: "Entering autonomous investigation mode...",
			}));
			await invoke("enable_autonomous_mode", {
				context: {
					url,
					title: document.title || "Current Page",
					content: body_text || "",
					puzzle_id: currentState.puzzleId,
					puzzle_clue: currentState.clue,
					target_pattern: "",
					hints: currentState.hints || [],
					hints_revealed: 0,
				},
			});
			console.log("[Ghost] Autonomous mode enabled");
		} catch (err) {
			console.error("[Ghost] Autonomous mode failed:", err);
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: "Autonomous mode disrupted...",
			}));
		}
	}, []);

	return {
		gameState,
		puzzles,
		isLoading,
		error,
		extensionConnected,
		captureAndAnalyze,
		generateDialogue,
		setClickable,
		showHint,
		advanceToNextPuzzle,
		resetGame,
		generateDynamicPuzzle,
		triggerDynamicPuzzle,
		startBackgroundChecks,
		enableAutonomousMode,
	};
}
