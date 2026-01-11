/**
 * @fileoverview Chrome Extension background service worker.
 * Handles Native Messaging connection to Tauri app and browser event forwarding.
 * @module background
 */

/**
 * @typedef {Object} NativeMessage
 * @property {string} type - Message type (page_load, tab_changed, page_content, etc.)
 * @property {string} [url] - Page URL
 * @property {string} [title] - Page title
 * @property {string} [body_text] - Page body text
 * @property {number} [timestamp] - Unix timestamp
 */

/**
 * @typedef {Object} EffectMessage
 * @property {string} action - Action type (inject_effect, highlight_text, etc.)
 * @property {string} [effect] - Effect name (glitch, scanlines, static, etc.)
 * @property {string} [text] - Text to highlight
 * @property {number} [duration] - Effect duration in ms
 */

/** Debug mode flag - set to false in production for performance */
const DEBUG_MODE = false;

/**
 * Conditional debug logger
 * @param {...any} args - Arguments to log
 */
function log(...args) {
	if (DEBUG_MODE) console.log("[OS Ghost]", ...args);
}

/**
 * Conditional warning logger
 * @param {...any} args - Arguments to log
 */
function warn(...args) {
	if (DEBUG_MODE) console.warn("[OS Ghost]", ...args);
}

/** Native messaging host name */
const NATIVE_HOST = "com.osghost.game";

/** @type {chrome.runtime.Port|null} */
let port = null;

/** @type {boolean} */
let isConnected = false;

/** @type {number} */
let reconnectAttempts = 0;

/** @type {number|null} */
let reconnectTimerId = null;

/** @type {number|null} */
let browsingContextTimerId = null;

/** Max reconnect attempts before giving up */
const MAX_RECONNECT_ATTEMPTS = 5;

/**
 * Update connection status in storage for popup
 * @param {boolean} connected
 */
function updateConnectionStatus(connected) {
	isConnected = connected;
	chrome.storage.local.set({ appConnected: connected });
}

/**
 * Fetch recent browsing history from Chrome history API
 * @param {number} [limit=50] - Maximum number of history items to fetch
 * @returns {Promise<Array<{url: string, title: string, visitCount: number, lastVisitTime: number}>>}
 */
async function fetchRecentHistory(limit = 50) {
	try {
		const oneWeekAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
		const history = await chrome.history.search({
			text: "",
			startTime: oneWeekAgo,
			maxResults: limit,
		});
		return history.map((item) => ({
			url: item.url || "",
			title: item.title || "",
			visitCount: item.visitCount || 1,
			lastVisitTime: item.lastVisitTime || Date.now(),
		}));
	} catch (error) {
		console.error("[OS Ghost] Failed to fetch history:", error);
		return [];
	}
}

/**
 * Fetch top sites (most visited) from Chrome topSites API
 * @returns {Promise<Array<{url: string, title: string}>>}
 */
async function fetchTopSites() {
	try {
		const topSites = await chrome.topSites.get();
		return topSites.slice(0, 10).map((site) => ({
			url: site.url || "",
			title: site.title || "",
		}));
	} catch (error) {
		console.error("[OS Ghost] Failed to fetch top sites:", error);
		return [];
	}
}

/**
 * Send browsing context (history + top sites) to native app
 * This enables immediate puzzle generation without waiting for page visits
 */
async function sendBrowsingContext() {
	let history = [];
	let topSites = [];

	// Fetch history with safety check
	try {
		// Verify API exists (permissions might be missing despite manifest)
		if (chrome.history && chrome.history.search) {
			history = await fetchRecentHistory(50);
		} else {
			console.warn(
				"[OS Ghost] chrome.history API is unavailable. Check permissions."
			);
		}
	} catch (error) {
		console.error("[OS Ghost] History fetch critical failure:", error);
	}

	// Fetch top sites with safety check
	try {
		if (chrome.topSites && chrome.topSites.get) {
			topSites = await fetchTopSites();
		} else {
			console.warn(
				"[OS Ghost] chrome.topSites API is unavailable. Check permissions."
			);
		}
	} catch (error) {
		console.error("[OS Ghost] TopSites fetch critical failure:", error);
	}

	log(
		"Sending browsing context:",
		history.length,
		"history,",
		topSites.length,
		"top sites"
	);

	sendToNative({
		type: "browsing_context",
		recent_history: history,
		top_sites: topSites,
		timestamp: Date.now(),
	});
}

/**
 * Helper to fetch and send content for a specific tab
 * @param {chrome.tabs.Tab} tab
 */
function fetchContentForTab(tab) {
	// Skip non-http URLs (chrome://, about:, edge://, file://, etc.)
	if (!tab?.id || !tab?.url) return;
	if (!tab.url.startsWith("http://") && !tab.url.startsWith("https://"))
		return;

	// Use try-catch to suppress errors when content script isn't available
	chrome.tabs.sendMessage(tab.id, { type: "get_content" }, (response) => {
		// Check for runtime error (content script not ready or restricted page)
		// This clears the error to prevent it from appearing in console
		const lastError = chrome.runtime.lastError;
		if (lastError) {
			// Expected on restricted pages - silently ignore
			return;
		}

		if (response?.bodyText) {
			sendToNative({
				type: "page_content",
				url: tab.url,
				body_text: response.bodyText.slice(0, 5000),
				timestamp: Date.now(),
			});
		}
	});
}

/**
 * Connect to the native messaging host.
 * Establishes connection to the Tauri app via native_bridge.
 * @returns {void}
 */
function connectToNative() {
	if (reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
		console.error("[OS Ghost] Max reconnect attempts reached");
		updateConnectionStatus(false);
		return;
	}

	try {
		port = chrome.runtime.connectNative(NATIVE_HOST);
		log("Connected to native host");
		updateConnectionStatus(true);
		reconnectAttempts = 0;

		// Fetch content immediately upon connection
		chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
			if (tabs[0]) {
				fetchContentForTab(tabs[0]);
			}
		});

		// Send browsing context (history + top sites) for immediate puzzle generation.
		// Added delay to prevent race condition with connection establishment.
		if (browsingContextTimerId) {
			clearTimeout(browsingContextTimerId);
		}
		browsingContextTimerId = setTimeout(() => {
			browsingContextTimerId = null;
			sendBrowsingContext();
		}, 1000);

		port.onMessage.addListener(handleNativeMessage);

		port.onDisconnect.addListener(() => {
			updateConnectionStatus(false);
			port = null;
			if (browsingContextTimerId) {
				clearTimeout(browsingContextTimerId);
				browsingContextTimerId = null;
			}
			const error = chrome.runtime.lastError;
			if (error) {
				console.error(
					"[OS Ghost] Native connection error:",
					error.message
				);
			} else {
				log("Native connection closed");
			}

			// Attempt to reconnect after delay
			if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
				reconnectAttempts++;
				log("Attempting to reconnect...");
				if (reconnectTimerId) {
					clearTimeout(reconnectTimerId);
				}
				reconnectTimerId = setTimeout(() => {
					reconnectTimerId = null;
					connectToNative();
				}, 5000);
			}
		});
	} catch (error) {
		console.error("[OS Ghost] Failed to connect:", error);
		updateConnectionStatus(false);
	}
}

/**
 * Handle messages from native app.
 * Routes commands to appropriate content script handlers.
 * @param {EffectMessage} message - Message from native app
 * @returns {void}
 */
function handleNativeMessage(message) {
	log("Received from native:", message);

	switch (message.action) {
		case "inject_effect":
			// Send effect to active tab's content script
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]?.id && tabs[0]?.url?.startsWith("http")) {
					chrome.tabs.sendMessage(
						tabs[0].id,
						{
							type: "effect",
							effect: message.effect,
							duration: message.duration || 1000,
						},
						() => {
							// Clear any error to prevent console noise
							void chrome.runtime.lastError;
						}
					);
				}
			});
			break;

		case "highlight_text":
			// Highlight specific text on page
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]?.id && tabs[0]?.url?.startsWith("http")) {
					chrome.tabs.sendMessage(
						tabs[0].id,
						{
							type: "highlight",
							text: message.text,
						},
						() => {
							void chrome.runtime.lastError;
						}
					);
				}
			});
			break;

		case "get_page_content":
			// Request page content from content script
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]?.id && tabs[0]?.url?.startsWith("http")) {
					chrome.tabs.sendMessage(
						tabs[0].id,
						{ type: "get_content" },
						(response) => {
							const lastError = chrome.runtime.lastError;
							if (lastError) return;

							if (port && response) {
								port.postMessage({
									type: "page_content",
									url: tabs[0].url,
									title: tabs[0].title,
									body_text: response.bodyText,
								});
							}
						}
					);
				}
			});
			break;

		case "navigate":
			// Force Ghost navigation (Computer Use)
			if (message.url) {
				chrome.tabs.query(
					{ active: true, currentWindow: true },
					(tabs) => {
						if (tabs[0]?.id) {
							chrome.tabs.update(tabs[0].id, {
								url: message.url,
							});
						}
					}
				);
			}
			break;

		case "acknowledged":
			// Native app acknowledged our message
			log("Message acknowledged");
			break;
	}
}

/**
 * Send message to native app.
 * @param {NativeMessage} message - Message to send
 * @returns {void}
 */
function sendToNative(message) {
	if (port && isConnected) {
		try {
			port.postMessage(message);
			log("Sent to native:", message);
		} catch (error) {
			console.error("[OS Ghost] Failed to send:", error);
			updateConnectionStatus(false);
		}
	} else {
		warn("Not connected to native host");
		connectToNative();
	}
}

// Listen for tab updates (page loads)
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
	if (changeInfo.status === "complete" && tab.url) {
		// Send page load event to native app
		sendToNative({
			type: "page_load",
			url: tab.url,
			title: tab.title || "",
			timestamp: Date.now(),
		});

		// Also fetch page content for deeper analysis
		fetchContentForTab(tab);
	}
});

// Listen for tab activation (switching tabs)
chrome.tabs.onActivated.addListener((activeInfo) => {
	chrome.tabs.get(activeInfo.tabId, (tab) => {
		if (tab?.url) {
			sendToNative({
				type: "tab_changed",
				url: tab.url,
				title: tab.title || "",
				timestamp: Date.now(),
			});

			// Fetch content on tab switch too
			fetchContentForTab(tab);
		}
	});
});


// Initialize connection on startup
updateConnectionStatus(false); // Start as disconnected
connectToNative();
log("Background service worker initialized");
