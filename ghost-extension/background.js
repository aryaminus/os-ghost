/**
 * OS Ghost - Background Service Worker
 * Connects Chrome to the native Ghost application via Native Messaging
 */

// Native host name (must match native-manifest.json)
const NATIVE_HOST = "com.osghost.game";

// Connection to native host
let port = null;
let isConnected = false;

/**
 * Connect to the native messaging host
 */
function connectToNative() {
	try {
		port = chrome.runtime.connectNative(NATIVE_HOST);
		isConnected = true;
		console.log("[OS Ghost] Connected to native host");

		// Handle messages from native app
		port.onMessage.addListener((message) => {
			console.log("[OS Ghost] Received from native:", message);
			handleNativeMessage(message);
		});

		// Handle disconnection
		port.onDisconnect.addListener(() => {
			isConnected = false;
			const error = chrome.runtime.lastError;
			if (error) {
				console.error(
					"[OS Ghost] Native connection error:",
					error.message
				);
			} else {
				console.log("[OS Ghost] Native connection closed");
			}

			// Attempt to reconnect after a delay
			setTimeout(() => {
				if (!isConnected) {
					console.log("[OS Ghost] Attempting to reconnect...");
					connectToNative();
				}
			}, 5000);
		});
	} catch (error) {
		console.error("[OS Ghost] Failed to connect to native host:", error);
	}
}

/**
 * Handle messages from the native Ghost app
 */
function handleNativeMessage(message) {
	switch (message.action) {
		case "inject_effect":
			// Send effect to active tab's content script
			chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
				if (tabs[0]) {
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
				if (tabs[0]) {
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
				if (tabs[0]) {
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
			console.log("[OS Ghost] Message acknowledged");
			break;
	}
}

/**
 * Send message to native app
 */
function sendToNative(message) {
	if (port && isConnected) {
		try {
			port.postMessage(message);
			console.log("[OS Ghost] Sent to native:", message);
		} catch (error) {
			console.error("[OS Ghost] Failed to send:", error);
			isConnected = false;
		}
	} else {
		console.warn("[OS Ghost] Not connected to native host");
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
	}
});

// Listen for tab activation (switching tabs)
chrome.tabs.onActivated.addListener((activeInfo) => {
	chrome.tabs.get(activeInfo.tabId, (tab) => {
		if (tab && tab.url) {
			sendToNative({
				type: "tab_changed",
				url: tab.url,
				title: tab.title || "",
				timestamp: Date.now(),
			});
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
connectToNative();
console.log("[OS Ghost] Background service worker initialized");
