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

/** Debug mode flag - set to true to enable verbose console logging */
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
 * Helper to fetch and send content for a specific tab
 * @param {chrome.tabs.Tab} tab
 */
function fetchContentForTab(tab) {
	// Skip non-http URLs (chrome://, about:, etc.)
	if (!tab?.id || !tab?.url || !tab.url.startsWith("http")) return;

	try {
		chrome.tabs.sendMessage(tab.id, { type: "get_content" }, (response) => {
			// Check for runtime error (e.g. content script not ready or restricted)
			if (chrome.runtime.lastError) {
				// Suppress expected errors on restricted pages
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
	} catch (e) {
		// Silently fail for restricted tabs
	}
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

		port.onMessage.addListener(handleNativeMessage);

		port.onDisconnect.addListener(() => {
			updateConnectionStatus(false);
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
				setTimeout(connectToNative, 5000);
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
				if (tabs[0]?.id) {
					chrome.tabs.sendMessage(tabs[0].id, {
						type: "effect",
						effect: message.effect,
						duration: message.duration || 1000,
					});
				}
			});
			break;

		case "highlight_text":
			// Highlight specific text on page
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]?.id) {
					chrome.tabs.sendMessage(tabs[0].id, {
						type: "highlight",
						text: message.text,
					});
				}
			});
			break;

		case "get_page_content":
			// Request page content from content script
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]?.id) {
					chrome.tabs.sendMessage(
						tabs[0].id,
						{ type: "get_content" },
						(response) => {
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

// Listen for messages from content scripts
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
	if (message.type === "page_content_response") {
		sendToNative({
			type: "page_content",
			url: sender.tab?.url || "",
			body_text: message.bodyText,
			timestamp: Date.now(),
		});
	}
	return false;
});

// Initialize connection on startup
updateConnectionStatus(false); // Start as disconnected
connectToNative();
log("Background service worker initialized");
