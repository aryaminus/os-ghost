/**
 * @fileoverview Popup script for OS Ghost Chrome extension.
 * Displays connection status and provides download link if app not running.
 */

/**
 * Update the popup UI based on connection status
 * @param {boolean} isConnected - Whether the app is connected
 */
function updateUI(isConnected) {
	const connectedStatus = document.getElementById("status-connected");
	const disconnectedStatus = document.getElementById("status-disconnected");
	const connectedMessage = document.getElementById("message-connected");
	const disconnectedMessage = document.getElementById("message-disconnected");
	const downloadBtn = document.getElementById("btn-download");

	if (isConnected) {
		connectedStatus.classList.remove("hidden");
		connectedMessage.classList.remove("hidden");
		disconnectedStatus.classList.add("hidden");
		disconnectedMessage.classList.add("hidden");
		downloadBtn.classList.add("hidden");
	} else {
		connectedStatus.classList.add("hidden");
		connectedMessage.classList.add("hidden");
		disconnectedStatus.classList.remove("hidden");
		disconnectedMessage.classList.remove("hidden");
		downloadBtn.classList.remove("hidden");
	}
}

/**
 * Check connection status from storage
 */
async function checkConnectionStatus() {
	try {
		const result = await chrome.storage.local.get(["appConnected"]);
		updateUI(result.appConnected === true);
	} catch (e) {
		console.error("[OS Ghost Popup] Error checking status:", e);
		updateUI(false);
	}
}

// Listen for storage changes to update UI in real-time
chrome.storage.onChanged.addListener((changes, namespace) => {
	if (namespace === "local" && changes.appConnected) {
		updateUI(changes.appConnected.newValue === true);
	}
});

// Check status when popup opens
document.addEventListener("DOMContentLoaded", checkConnectionStatus);
