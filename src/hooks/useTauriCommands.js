/**
 * @fileoverview React hooks for managing Ghost game state and Tauri IPC commands.
 * Provides the main interface between React frontend and Rust backend.
 * @module useTauriCommands
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback, useRef } from "react";

/** Debug mode - only log in development */
const DEBUG_MODE = import.meta.env.DEV;

/**
 * Conditional debug logger
 * @param {...any} args - Arguments to log
 */
function log(...args) {
	if (DEBUG_MODE) console.log("[Ghost]", ...args);
}

/**
 * Conditional warning logger
 * @param {...any} args - Arguments to log
 */
function warn(...args) {
	if (DEBUG_MODE) console.warn("[Ghost]", ...args);
}

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
 * @property {boolean} isLoading - Whether an async operation is in progress
 * @property {string|null} error - Error message if any
 * @property {function(): Promise<string|null>} captureAndAnalyze - Capture screen and analyze with AI
 * @property {function(): Promise<void>} showHint - Show next available hint
 * @property {function(): void} advanceToNextPuzzle - Move to next puzzle
 * @property {function(): Promise<void>} resetGame - Reset game progress
 */

/** @type {GameState} */
export const initialGameState = {
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
	const [isLoading, setIsLoading] = useState(false);
	/** @type {[string|null, function(string|null): void]} */
	const [error, setError] = useState(null);

	// System status for Chrome/extension detection
	const [systemStatus, setSystemStatus] = useState({
		chromeInstalled: null, // null = checking, true/false = detected
		chromePath: null,
		extensionConnected: false,
		extensionOperational: false,
		apiKeyConfigured: false,
		lastKnownUrl: null,
		currentMode: "game",
	});

	/** @type {React.MutableRefObject<PageContentPayload|null>} */
	const latestContentRef = useRef(globalLatestContent);
	/** @type {React.MutableRefObject<GameState>} */
	const gameStateRef = useRef(gameState);

	// Track last processed URL to debounce agent cycles
	const lastProcessedUrlRef = useRef(null);
	// Lock to prevent concurrent agent cycles
	const agentCycleInProgressRef = useRef(false);
	// Debounce timer for navigation events
	const navigationDebounceRef = useRef(null);

	/**
	 * Detect system status (Chrome, extension, etc.)
	 * Called on mount and periodically
	 */
	const detectSystemStatus = useCallback(async () => {
		try {
			const status = await invoke("detect_chrome");
			setSystemStatus((prev) => ({
				...prev,
				chromeInstalled: status.chrome_installed,
				chromePath: status.chrome_path,
				apiKeyConfigured: status.api_key_configured,
				currentMode: status.current_mode || "game",
			}));
		} catch (err) {
			console.error("[Ghost] System detection failed:", err);
		}
	}, []);

	// Detect system status on mount (initial check)
	useEffect(() => {
		detectSystemStatus();
	}, [detectSystemStatus]);

	// Companion behavior state
	const [companionBehavior, setCompanionBehavior] = useState(null);

	/**
	 * Generate an adaptive puzzle based on activity history
	 */
	const generateAdaptivePuzzle = useCallback(async () => {
		setIsLoading(true);
		try {
			const puzzle = await invoke("generate_adaptive_puzzle");
			log("Generated adaptive puzzle:", puzzle);

			setGameState((prev) => ({
				...prev,
				state: "thinking",
				clue: puzzle.clue,
				puzzleId: `adaptive_${Date.now()}`,
				currentPuzzle: puzzle.target_description,
				hint: puzzle.hints?.[0] || "",
				hints: puzzle.hints || [],
				hintsRevealed: 0,
				hintAvailable: false,
				proximity: 0.2,
				dialogue: `Inspired by your ${puzzle.inspired_by}... ${puzzle.clue}`,
			}));

			setCompanionBehavior(null); // Clear suggestion
		} catch (err) {
			console.error("[Ghost] Adaptive puzzle failed:", err);
			setError(
				"Could not generate adaptive puzzle. Need more observations."
			);
		} finally {
			setIsLoading(false);
		}
	}, []);

	/**
	 * Trigger a visual effect in the browser.
	 * @param {string} effect - Effect name (glitch, scanlines, static, flash, start_trail, stop_trail)
	 * @param {number} [duration] - Effect duration in ms
	 * @param {string} [text] - Optional text to highlight
	 * @returns {Promise<void>}
	 */
	const triggerBrowserEffect = useCallback(async (effect, duration, text) => {
		try {
			await invoke("trigger_browser_effect", {
				effect,
				duration,
				text,
			});
		} catch (err) {
			console.error("[Ghost] Failed to trigger effect:", err);
		}
	}, []);

	// Manage Ghost Trail effect based on state
	useEffect(() => {
		if (gameState.state === "searching" || gameState.state === "thinking") {
			triggerBrowserEffect("start_trail");
		} else {
			triggerBrowserEffect("stop_trail");
		}
	}, [gameState.state, triggerBrowserEffect]);

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
		log(" handlePageContent received:", payload.url);
		globalLatestContent = payload;
		latestContentRef.current = payload;

		// If no puzzle is active and API key is configured, generate one!
		const currentState = gameStateRef.current;
		if (!currentState.puzzleId && currentState.apiKeyConfigured) {
			log(
				"[Ghost] No active puzzle, triggering generation from content..."
			);

			// Use ref to avoid hoisting issues with triggerDynamicPuzzle being defined later
			triggerPuzzleGenerationRef.current &&
				triggerPuzzleGenerationRef.current();
		}
	}, []);

	/**
	 * Wrapper to generate puzzle from current context
	 */
	const triggerDynamicPuzzle = useCallback(async () => {
		log("[Ghost] Requesting investigation from backend...");

		try {
			const puzzle = await invoke("start_investigation");

			if (puzzle) {
				log(
					"[Ghost] Investigation complete, puzzle received:",
					puzzle.id
				);
				setGameState((prev) => ({
					...prev,
					puzzleId: puzzle.id,
					clue: puzzle.clue,
					hint: puzzle.hint || "",
					hints: puzzle.hints || [],
					hintsRevealed: 0,
					hintAvailable: false,
					state: "thinking", // Start thinking then idle?
					dialogue: puzzle.clue, // Use clue as dialogue initially
					currentPuzzle: puzzle.target_description,
					proximity: 0,
				}));
			}
		} catch (err) {
			console.error("[Ghost] Investigation failed:", err);
			// Fallback or just log
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: "Nothing found here. Keep browsing.",
			}));
		}
	}, []);
	// Load persistent state and check API key on mount
	useEffect(() => {
		initializeGame();
	}, []);

	// Listen for hint_available event instead of polling
	// This is handled in the main event listener effect below

	const handleNavigationRef = useRef(null);
	const handlePageContentRef = useRef(null);
	const advanceToNextPuzzleRef = useRef(null);
	const triggerPuzzleGenerationRef = useRef(null);

	// Update ref when function changes
	useEffect(() => {
		triggerPuzzleGenerationRef.current = triggerDynamicPuzzle;
	}, [triggerDynamicPuzzle]);

	useEffect(() => {
		handlePageContentRef.current = handlePageContent;
	}, [handlePageContent]);

	// Listen for browser navigation events from Chrome extension
	// Only set up once on mount to avoid re-subscribing
	useEffect(() => {
		// Store unlisten functions
		const unlistenFns = [];
		let isUnmounting = false;

		log(" Setting up Tauri event listeners...");

		const setupListeners = async () => {
			const register = async (event, callback) => {
				const unlisten = await listen(event, callback);
				if (isUnmounting) {
					unlisten();
				} else {
					unlistenFns.push(unlisten);
				}
			};

			await register("browser_navigation", (event) => {
				log(
					"[Ghost] Received browser_navigation event:",
					event.payload
				);
				handleNavigationRef.current?.(
					/** @type {NavigationPayload} */ (event.payload)
				);
			});

			await register("page_content", (event) => {
				log("[Ghost] Received page_content event:", event.payload);
				handlePageContentRef.current?.(
					/** @type {PageContentPayload} */ (event.payload)
				);
			});

			await register("browsing_context", (event) => {
				const { recent_history, top_sites } = event.payload;
				log(
					"[Ghost] Received browsing context:",
					recent_history?.length,
					"history,",
					top_sites?.length,
					"top sites"
				);

				// If no puzzle and no page content, generate from history!
				const currentState = gameStateRef.current;
				if (
					!currentState.puzzleId &&
					currentState.apiKeyConfigured &&
					!latestContentRef.current
				) {
					log("[Ghost] Auto-triggering history puzzle generation...");
					triggerPuzzleGenerationRef.current &&
						triggerPuzzleGenerationRef.current();
				}
			});

			// Test if events work by emitting a test event from backend
			try {
				const testResult = await invoke("check_api_key");
				log(
					"[Ghost] Backend connection verified, API key configured:",
					testResult
				);
			} catch (err) {
				console.error("[Ghost] Backend connection test failed:", err);
			}

			await register("extension_connected", () => {
				log(" Extension connected");
				setSystemStatus((prev) => ({
					...prev,
					extensionConnected: true,
					extensionOperational: true,
				}));
			});

			await register("extension_disconnected", () => {
				log(" Extension disconnected");
				setSystemStatus((prev) => ({
					...prev,
					extensionConnected: false,
					extensionOperational: false,
				}));
			});

			await register("system_status_update", (event) => {
				const status = event.payload;
				setSystemStatus((prev) => ({
					...prev,
					chromeInstalled: status.chrome_installed,
					chromePath: status.chrome_path,
					apiKeyConfigured: status.api_key_configured,
					currentMode: status.current_mode || "game",
					// Preserve extension connection state as it might be handled separately
					// or merge if backend sends it authoritative
				}));
			});

			await register("autonomous_progress", (event) => {
				const { proximity, message, solved, finished } = event.payload;
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
					setTimeout(() => advanceToNextPuzzleRef.current(), 5000);
				}
			});

			await register("companion_behavior", (event) => {
				const behavior = event.payload;
				log("Companion behavior:", behavior);
				setCompanionBehavior(behavior);
				// Clear behavior after 30s - timeout cleaned up via isUnmounting check
				const timeoutId = setTimeout(() => {
					if (!isUnmounting) {
						setCompanionBehavior(null);
					}
				}, 30000);
				// Store for potential cleanup (though register handles unlisten)
				unlistenFns.push(() => clearTimeout(timeoutId));
			});

			await register("hint_available", (event) => {
				log("[Ghost] Hint available event received");
				setGameState((prev) => ({ ...prev, hintAvailable: true }));
			});

			await register("ghost_observation", (event) => {
				log("[Ghost] Received observation:", event.payload);
				const observation = event.payload;

				// Logic merged from both listeners
				// 1. Contextual comment if no puzzle active (using ref to avoid dep)
				if (observation.puzzle_theme && !gameStateRef.current.clue) {
					setGameState((prev) => ({
						...prev,
						dialogue: `I sense something about ${observation.puzzle_theme}...`,
					}));
				}

				// 2. Observant state update
				if (
					observation &&
					observation.activity &&
					!observation.is_idle
				) {
					setGameState((prev) => ({
						...prev,
						dialogue: `I see you: ${observation.activity}`,
						state: "observant",
					}));

					// Reset to idle after 10s - with cleanup on unmount
					const observantTimeoutId = setTimeout(() => {
						if (!isUnmounting) {
							setGameState((prev) =>
								prev.state === "observant"
									? { ...prev, state: "idle" }
									: prev
							);
						}
					}, 10000);
					unlistenFns.push(() => clearTimeout(observantTimeoutId));
				}
			});

			if (!isUnmounting) {
				log(" Event listeners registered successfully");
			}
		};

		setupListeners();

		return () => {
			isUnmounting = true;
			unlistenFns.forEach((fn) => fn());
			// Clean up navigation debounce timer
			if (navigationDebounceRef.current) {
				clearTimeout(navigationDebounceRef.current);
				navigationDebounceRef.current = null;
			}
		};
	}, []);

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
				currentPuzzle: savedState?.current_puzzle_index || 0,
				dialogue:
					"Connection established. I need to see what you see... browse to a page.",
				// Ensure we don't start with a default puzzle
				puzzleId: "",
				clue: "",
			}));

			// If we already have content (re-mount), trigger generation
			if (latestContentRef.current && configured) {
				log(
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
		log(" Puzzle solved! Preparing next mystery...");

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
		// Always try to trigger next puzzle (will fallback to history if no content)
		if (triggerPuzzleGenerationRef.current) {
			setTimeout(() => {
				log("[Ghost] Auto-triggering next puzzle generation...");
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

			// Debounce: skip if same URL was just processed
			if (lastProcessedUrlRef.current === url) {
				log(" Skipping duplicate navigation for:", url);
				return;
			}

			// Skip if another agent cycle is in progress
			if (agentCycleInProgressRef.current) {
				log(" Agent cycle in progress, debouncing navigation");
				// Clear any pending debounce and schedule new one
				if (navigationDebounceRef.current) {
					clearTimeout(navigationDebounceRef.current);
				}
				navigationDebounceRef.current = setTimeout(() => {
					handleNavigationRef.current?.(payload);
				}, 500);
				return;
			}

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
				log(" No active puzzle, waiting for content...");
				return;
			}

			// Set lock and track URL
			agentCycleInProgressRef.current = true;
			lastProcessedUrlRef.current = url;

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
			} finally {
				// Release lock after short delay to allow batching
				setTimeout(() => {
					agentCycleInProgressRef.current = false;
				}, 300);
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
	 * Verify if the current screen matches the puzzle clue.
	 * @returns {Promise<Object|null>} Verification result
	 */
	const verifyScreenshotProof = async () => {
		const currentState = gameStateRef.current;
		if (!currentState.apiKeyConfigured || !currentState.puzzleId) {
			return null;
		}

		setIsLoading(true);
		setGameState((prev) => ({
			...prev,
			state: "thinking",
			dialogue: "Analyzing your proof...",
		}));

		try {
			const result = await invoke("verify_screenshot_proof", {
				puzzleId: currentState.puzzleId,
			});

			log(" Verification result:", result);

			if (result.found) {
				triggerBrowserEffect("flash", 1000); // Visual feedback
				setGameState((prev) => ({
					...prev,
					state: "celebrate",
					dialogue:
						result.explanation ||
						"Proof accepted! You found the fragment.",
					proximity: 1.0,
				}));
				// Advance after delay
				setTimeout(() => advanceToNextPuzzle(), 4000);
			} else {
				setGameState((prev) => ({
					...prev,
					state: "idle",
					dialogue:
						result.explanation ||
						"That doesn't look like the solution...",
				}));
			}

			return result;
		} catch (err) {
			console.error("[Ghost] Verification failed:", err);
			setGameState((prev) => ({
				...prev,
				state: "idle",
				dialogue: "I couldn't verify that visual...",
			}));
			return null;
		} finally {
			setIsLoading(false);
		}
	};

	/**
	 * Show the next available hint if timer has elapsed.
	 * Progressive hints unlock at 60s intervals.
	 * @returns {Promise<void>}
	 */
	const showHint = useCallback(async () => {
		const currentState = gameStateRef.current;
		if (!currentState.hintAvailable) {
			setGameState((prev) => ({
				...prev,
				dialogue: "Patience... the memories need time to surface.",
			}));
			return;
		}

		try {
			// Get puzzle's hints from state (puzzles array may be empty for dynamic puzzles)
			const hints =
				currentState.hints.length > 0
					? currentState.hints
					: currentState.hint
						? [currentState.hint]
						: [];

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
	}, []);

	/**
	 * Reset game progress and start fresh.
	 * Clears all solved puzzles and discoveries.
	 * @returns {Promise<void>}
	 */
	const resetGame = useCallback(async () => {
		try {
			await invoke("reset_game");
			// await initializeGame(); // Don't re-initialize, it triggers auto-generation if content exists

			// Manually reset local state
			setGameState((prev) => ({
				...initialGameState,
				apiKeyConfigured: prev.apiKeyConfigured, // Keep API key
				dialogue: "Memory wiped. Ready for a new beginning...",
				state: "idle",
			}));

			// Reset refs to allow new generation

			lastProcessedUrlRef.current = null;
		} catch (err) {
			console.error("[Ghost] Failed to reset game:", err);
		}
	}, []);

	/**
	 * Generate a dynamic puzzle based on current page content.
	 * Creates unique contextual puzzles from what the user is viewing.
	 * @param {string} url - Current page URL
	 * @param {string} title - Current page title
	 * @param {string} content - Page body text (first 500 chars)
	 * @returns {Promise<Object|null>} Generated puzzle or null if failed
	 */

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
		log(" startBackgroundChecks called", {
			hasContent: !!latestContentRef.current,
			apiKeyConfigured: currentState.apiKeyConfigured,
		});

		if (!currentState.apiKeyConfigured) {
			warn(
				"[Ghost] Cannot run background checks: API key not configured"
			);
			return;
		}

		if (!latestContentRef.current) {
			warn(
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
			log(" Background checks completed");
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
		log(" enableAutonomousMode called", {
			hasContent: !!latestContentRef.current,
			apiKeyConfigured: currentState.apiKeyConfigured,
		});

		if (!currentState.apiKeyConfigured) {
			warn(
				"[Ghost] Cannot enable autonomous mode: API key not configured"
			);
			return;
		}

		if (!latestContentRef.current) {
			warn(
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
			log(" Autonomous mode enabled");
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
		isLoading,
		extensionConnected: systemStatus.extensionConnected,
		systemStatus,
		companionBehavior,
		captureAndAnalyze,
		verifyScreenshotProof,
		triggerBrowserEffect,
		showHint,
		advanceToNextPuzzle,
		resetGame,
		triggerDynamicPuzzle,
		startBackgroundChecks,
		enableAutonomousMode,
		detectSystemStatus,
		generateAdaptivePuzzle,
	};
}
