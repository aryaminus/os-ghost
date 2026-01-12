/**
 * @fileoverview Shared data and IPC utilities for the frontend.
 * @module utils/data
 */

import { invoke } from "@tauri-apps/api/core";

/** Debug mode - only log in development */
const DEBUG_MODE = import.meta.env.DEV;

/**
 * Helper to safely invoke Tauri commands with error handling.
 * @template T
 * @param {string} command - Command name
 * @param {Object} [args] - Arguments
 * @param {T} [fallback=null] - Fallback value on error
 * @returns {Promise<T>} Result or fallback
 */
export async function safeInvoke(command, args = {}, fallback = null) {
	try {
		return await invoke(command, args);
	} catch (err) {
		console.error(`[Ghost] Command '${command}' failed:`, err);
		return fallback;
	}
}

/**
 * Conditional debug logger
 * @param {...any} args - Arguments to log
 */
export function log(...args) {
	if (DEBUG_MODE) console.log("[Ghost]", ...args);
}

/**
 * Conditional warning logger
 * @param {...any} args - Arguments to log
 */
export function warn(...args) {
	if (DEBUG_MODE) console.warn("[Ghost]", ...args);
}
