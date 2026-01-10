/**
 * @fileoverview Agent loop and autonomous mode hook.
 * Handles browser events, agent cycles, and autonomous investigation.
 * @module useAgentLoop
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback, useRef } from "react";

/** Debug mode - only log in development */
const DEBUG_MODE = import.meta.env.DEV;

function log(...args) {
	if (DEBUG_MODE) console.log("[AgentLoop]", ...args);
}

function warn(...args) {
	if (DEBUG_MODE) console.warn("[AgentLoop]", ...args);
}

// Global cache to persist content across hook re-mounts
let globalLatestContent = null;

/**
 * Page content from Chrome extension
 * @typedef {Object} PageContent
 * @property {string} url - Page URL
 * @property {string} body_text - Page body text
 * @property {number} timestamp - Unix timestamp
 */

/**
 * Hook for managing agent loops, browser events, and autonomous mode.
 * Connects to Chrome extension events and triggers agent processing.
 *
 * @param {Object} options - Configuration options
 * @param {Function} options.getGameState - Function to get current game state
 * @param {Function} options.updateGameState - Function to update game state
 * @param {Function} options.loadPuzzle - Function to load a new puzzle
 * @param {Function} options.advancePuzzle - Function to advance to next puzzle
 * @param {Function} options.applyOrchestrationResult - Function to apply agent results
 * @returns {Object} Agent loop controls and status
 */
export function useAgentLoop({
	getGameState,
	updateGameState,
	loadPuzzle,
	advancePuzzle,
	applyOrchestrationResult,
}) {
	const [isProcessing, setIsProcessing] = useState(false);
	const [companionBehavior, setCompanionBehavior] = useState(null);

	// Refs for tracking state
	const latestContentRef = useRef(globalLatestContent);
	const lastProcessedUrlRef = useRef(null);
	const agentCycleInProgressRef = useRef(false);
	const navigationDebounceRef = useRef(null);
	const isMountedRef = useRef(true);

	// Refs for callbacks to avoid stale closures
	const handleNavigationRef = useRef(null);
	const handlePageContentRef = useRef(null);
	const triggerPuzzleGenerationRef = useRef(null);
	const advancePuzzleRef = useRef(advancePuzzle);

	// Refs for timeout cleanup to prevent memory leaks
	const advanceTimeoutRef = useRef(null);
	const lockReleaseTimeoutRef = useRef(null);

	// Keep refs in sync
	advancePuzzleRef.current = advancePuzzle;

	/**
	 * Get latest page content
	 */
	const getLatestContent = useCallback(() => latestContentRef.current, []);

	/**
	 * Generate a new puzzle from current context
	 */
	const triggerDynamicPuzzle = useCallback(async () => {
		log("Requesting investigation from backend...");

		try {
			const puzzle = await invoke("start_investigation");
			if (puzzle && isMountedRef.current) {
				log("Investigation complete, puzzle received:", puzzle.id);
				loadPuzzle(puzzle);
			}
		} catch (err) {
			console.error("[AgentLoop] Investigation failed:", err);
			updateGameState({
				state: "idle",
				dialogue: "Nothing found here. Keep browsing.",
			});
		}
	}, [loadPuzzle, updateGameState]);

	// Update ref
	triggerPuzzleGenerationRef.current = triggerDynamicPuzzle;

	/**
	 * Handle page content from extension
	 * @param {PageContent} payload
	 */
	const handlePageContent = useCallback((payload) => {
		log("handlePageContent received:", payload.url);
		globalLatestContent = payload;
		latestContentRef.current = payload;

		// If no puzzle active and API key configured, generate one
		const currentState = getGameState();
		if (!currentState.puzzleId && currentState.apiKeyConfigured) {
			log("No active puzzle, triggering generation from content...");
			triggerPuzzleGenerationRef.current?.();
		}
	}, [getGameState]);

	handlePageContentRef.current = handlePageContent;

	/**
	 * Handle browser navigation event
	 * @param {Object} payload - Navigation event
	 */
	const handleNavigation = useCallback(async (payload) => {
		const { url, title } = payload;
		const currentState = getGameState();

		// Debounce duplicate URLs
		if (lastProcessedUrlRef.current === url) {
			log("Skipping duplicate navigation for:", url);
			return;
		}

		// Skip if agent cycle in progress
		if (agentCycleInProgressRef.current) {
			log("Agent cycle in progress, debouncing navigation");
			if (navigationDebounceRef.current) {
				clearTimeout(navigationDebounceRef.current);
			}
			navigationDebounceRef.current = setTimeout(() => {
				handleNavigationRef.current?.(payload);
			}, 500);
			return;
		}

		updateGameState({ currentUrl: url, state: "thinking" });

		// Skip if API key not configured
		if (!currentState.apiKeyConfigured) {
			updateGameState({ state: "idle" });
			return;
		}

		// Skip if no puzzle active
		if (!currentState.puzzleId) {
			log("No active puzzle, waiting for content...");
			return;
		}

		// Set lock and track URL
		agentCycleInProgressRef.current = true;
		lastProcessedUrlRef.current = url;

		try {
			setIsProcessing(true);
			const result = await invoke("process_agent_cycle", {
				context: {
					url,
					title,
					content: latestContentRef.current?.body_text || "",
					puzzle_id: currentState.puzzleId,
					puzzle_clue: currentState.clue,
					target_pattern: "",
					hints: currentState.hints,
					hints_revealed: currentState.hintsRevealed || 0,
				},
			});

			if (result && isMountedRef.current) {
				const solved = applyOrchestrationResult(result);
				if (solved) {
					// Clear any existing timeout before setting a new one
					if (advanceTimeoutRef.current) {
						clearTimeout(advanceTimeoutRef.current);
					}
					advanceTimeoutRef.current = setTimeout(() => advancePuzzleRef.current?.(), 5000);
				}
			}
		} catch (err) {
			console.error("[AgentLoop] Agent cycle failed:", err);
			updateGameState({ state: "idle" });
		} finally {
			if (isMountedRef.current) {
				setIsProcessing(false);
			}
			// Release lock after short delay (with cleanup tracking)
			if (lockReleaseTimeoutRef.current) {
				clearTimeout(lockReleaseTimeoutRef.current);
			}
			lockReleaseTimeoutRef.current = setTimeout(() => {
				agentCycleInProgressRef.current = false;
			}, 300);
		}
	}, [getGameState, updateGameState, applyOrchestrationResult]);

	handleNavigationRef.current = handleNavigation;

	/**
	 * Run background checks (Parallel Workflow)
	 */
	const startBackgroundChecks = useCallback(async () => {
		const currentState = getGameState();

		if (!currentState.apiKeyConfigured) {
			warn("Cannot run background checks: API key not configured");
			return;
		}

		if (!latestContentRef.current) {
			warn("Cannot run background checks: No page content");
			updateGameState({
				dialogue: "No page content detected. Browse to a page first...",
			});
			return;
		}

		if (!currentState.puzzleId) {
			updateGameState({
				dialogue: "No mystery to investigate yet... waiting for signal.",
			});
			return;
		}

		const { url, body_text } = latestContentRef.current;
		try {
			updateGameState({
				state: "searching",
				dialogue: "Scanning the digital ether...",
			});
			await invoke("start_background_checks", {
				context: {
					url,
					title: document.title || "Current Page",
					content: body_text || "",
					puzzle_id: currentState.puzzleId,
					puzzle_clue: currentState.clue,
					target_pattern: "",
					hints: currentState.hints || [],
					hints_revealed: currentState.hintsRevealed || 0,
				},
			});
			log("Background checks completed");
		} catch (err) {
			console.error("[AgentLoop] Background checks failed:", err);
			updateGameState({
				state: "idle",
				dialogue: "Background scan encountered interference...",
			});
		}
	}, [getGameState, updateGameState]);

	/**
	 * Enable autonomous mode (Loop Workflow)
	 */
	const enableAutonomousMode = useCallback(async () => {
		const currentState = getGameState();

		if (!currentState.apiKeyConfigured) {
			warn("Cannot enable autonomous mode: API key not configured");
			return;
		}

		if (!latestContentRef.current) {
			warn("Cannot enable autonomous mode: No page content");
			updateGameState({
				dialogue: "No page content detected. Browse to a page first...",
			});
			return;
		}

		if (!currentState.puzzleId) {
			updateGameState({
				dialogue: "I cannot work autonomously without a target...",
			});
			return;
		}

		const { url, body_text } = latestContentRef.current;

		try {
			updateGameState({
				state: "thinking",
				dialogue: "Entering autonomous investigation mode...",
			});
			await invoke("enable_autonomous_mode", {
				context: {
					url,
					title: document.title || "Current Page",
					content: body_text || "",
					puzzle_id: currentState.puzzleId,
					puzzle_clue: currentState.clue,
					target_pattern: "",
					hints: currentState.hints || [],
					hints_revealed: currentState.hintsRevealed || 0,
				},
			});
			log("Autonomous mode enabled");
		} catch (err) {
			console.error("[AgentLoop] Autonomous mode failed:", err);
			updateGameState({
				state: "idle",
				dialogue: "Autonomous mode disrupted...",
			});
		}
	}, [getGameState, updateGameState]);

	// Set up event listeners
	useEffect(() => {
		isMountedRef.current = true;
		const unlistenFns = [];
		let isUnmounting = false;

		log("Setting up agent loop event listeners...");

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
				log("Received browser_navigation event:", event.payload);
				handleNavigationRef.current?.(event.payload);
			});

			await register("page_content", (event) => {
				log("Received page_content event:", event.payload);
				handlePageContentRef.current?.(event.payload);
			});

			await register("browsing_context", (event) => {
				const { recent_history, top_sites } = event.payload;
				log("Received browsing context:", recent_history?.length, "history,", top_sites?.length, "top sites");

				const currentState = getGameState();
				if (!currentState.puzzleId && currentState.apiKeyConfigured && !latestContentRef.current) {
					log("Auto-triggering history puzzle generation...");
					triggerPuzzleGenerationRef.current?.();
				}
			});

			await register("autonomous_progress", (event) => {
				const { proximity, message, solved, finished } = event.payload;
				updateGameState({
					proximity,
					dialogue: message,
					state: solved ? "celebrate" : finished ? "idle" : "searching",
				});

				if (solved) {
					// Clear any existing timeout before setting a new one
					if (advanceTimeoutRef.current) {
						clearTimeout(advanceTimeoutRef.current);
					}
					advanceTimeoutRef.current = setTimeout(() => advancePuzzleRef.current?.(), 5000);
				}
			});

			await register("companion_behavior", (event) => {
				log("Companion behavior:", event.payload);
				setCompanionBehavior(event.payload);
				const timeoutId = setTimeout(() => {
					if (!isUnmounting) {
						setCompanionBehavior(null);
					}
				}, 30000);
				unlistenFns.push(() => clearTimeout(timeoutId));
			});

			await register("hint_available", () => {
				log("Hint available event received");
				updateGameState({ hintAvailable: true });
			});

			await register("ghost_observation", (event) => {
				log("Received observation:", event.payload);
				const observation = event.payload;
				const currentState = getGameState();

				if (observation.puzzle_theme && !currentState.clue) {
					updateGameState({
						dialogue: `I sense something about ${observation.puzzle_theme}...`,
					});
				}

				if (observation && observation.activity && !observation.is_idle) {
					updateGameState({
						dialogue: `I see you: ${observation.activity}`,
						state: "observant",
					});

					const timeoutId = setTimeout(() => {
						if (!isUnmounting) {
							const state = getGameState();
							if (state.state === "observant") {
								updateGameState({ state: "idle" });
							}
						}
					}, 10000);
					unlistenFns.push(() => clearTimeout(timeoutId));
				}
			});

			if (!isUnmounting) {
				log("Event listeners registered successfully");
			}
		};

		setupListeners();

		return () => {
			isUnmounting = true;
			isMountedRef.current = false;
			unlistenFns.forEach((fn) => fn());
			if (navigationDebounceRef.current) {
				clearTimeout(navigationDebounceRef.current);
			}
			// Clean up timeout refs to prevent memory leaks and state updates on unmounted component
			if (advanceTimeoutRef.current) {
				clearTimeout(advanceTimeoutRef.current);
			}
			if (lockReleaseTimeoutRef.current) {
				clearTimeout(lockReleaseTimeoutRef.current);
			}
		};
	}, [getGameState, updateGameState]);

	return {
		isProcessing,
		companionBehavior,
		// Content
		getLatestContent,
		// Actions
		triggerDynamicPuzzle,
		startBackgroundChecks,
		enableAutonomousMode,
		// Clear companion behavior
		clearCompanionBehavior: useCallback(() => setCompanionBehavior(null), []),
	};
}
