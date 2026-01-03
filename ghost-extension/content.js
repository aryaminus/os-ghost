/**
 * @fileoverview Chrome Extension content script.
 * Runs on every page to enable visual effects and content extraction for OS Ghost.
 * @module content
 */
"use strict";

/**
 * @typedef {Object} PageContent
 * @property {string} bodyText - Page body text (first 5000 chars)
 * @property {string} title - Page title
 * @property {string} url - Page URL
 */

/**
 * @typedef {"glitch" | "scanlines" | "static" | "pulse" | "flicker"} EffectType
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

/** @type {Set<string>} Track active effects to prevent duplicates */
const activeEffects = new Set();

/**
 * Apply visual effect to the page.
 * @param {EffectType} effect - Effect type to apply
 * @param {number} duration - Effect duration in milliseconds
 * @returns {void}
 */
function applyEffect(effect, duration) {
	const effectId = effect + "_" + Date.now();
	activeEffects.add(effectId);

	switch (effect) {
		case "glitch":
			applyGlitchEffect(duration);
			break;
		case "scanlines":
			applyScanlines(duration);
			break;
		case "static":
			applyStaticNoise(duration);
			break;
		case "pulse":
			applyPulseGlow(duration);
			break;
		case "flicker":
			applyFlicker(duration);
			break;
		default:
			warn("Unknown effect:", effect);
	}

	setTimeout(() => {
		activeEffects.delete(effectId);
	}, duration);
}

/**
 * Apply glitch distortion effect to page.
 * Creates clip-path animation that distorts the view.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyGlitchEffect(duration) {
	const style = document.createElement("style");
	style.id = "ghost-glitch-style";
	style.textContent = `
    @keyframes ghost-glitch {
      0%, 100% { 
        clip-path: inset(0 0 0 0);
        transform: translate(0);
      }
      20% { 
        clip-path: inset(20% 0 30% 0);
        transform: translate(-5px, 0);
      }
      40% { 
        clip-path: inset(50% 0 20% 0);
        transform: translate(5px, 0);
      }
      60% { 
        clip-path: inset(10% 0 60% 0);
        transform: translate(-3px, 0);
      }
      80% { 
        clip-path: inset(40% 0 10% 0);
        transform: translate(3px, 0);
      }
    }
    .ghost-glitch-active {
      animation: ghost-glitch 0.1s infinite;
    }
  `;
	document.head.appendChild(style);
	document.body.classList.add("ghost-glitch-active");

	setTimeout(() => {
		document.body.classList.remove("ghost-glitch-active");
		style.remove();
	}, duration);
}

/**
 * Apply CRT scanlines overlay effect.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyScanlines(duration) {
	const overlay = document.createElement("div");
	overlay.id = "ghost-scanlines";
	overlay.style.cssText = `
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: repeating-linear-gradient(
      0deg,
      rgba(0, 0, 0, 0.1) 0px,
      rgba(0, 0, 0, 0.1) 1px,
      transparent 1px,
      transparent 2px
    );
    pointer-events: none;
    z-index: 999999;
    animation: ghost-scanlines-flicker 0.1s infinite;
  `;

	const style = document.createElement("style");
	style.id = "ghost-scanlines-style";
	style.textContent = `
    @keyframes ghost-scanlines-flicker {
      0%, 100% { opacity: 0.8; }
      50% { opacity: 0.5; }
    }
  `;

	document.head.appendChild(style);
	document.body.appendChild(overlay);

	setTimeout(() => {
		overlay.remove();
		style.remove();
	}, duration);
}

/**
 * Apply TV static noise effect using canvas.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyStaticNoise(duration) {
	const canvas = document.createElement("canvas");
	canvas.id = "ghost-static";
	canvas.style.cssText = `
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 999999;
    opacity: 0.1;
  `;
	document.body.appendChild(canvas);

	const ctx = canvas.getContext("2d");
	canvas.width = window.innerWidth;
	canvas.height = window.innerHeight;

	/** @type {number} */
	let animationId;

	/**
	 * Draw random noise to canvas.
	 */
	function drawNoise() {
		const imageData = ctx.createImageData(canvas.width, canvas.height);
		for (let i = 0; i < imageData.data.length; i += 4) {
			const noise = Math.random() * 255;
			imageData.data[i] = noise;
			imageData.data[i + 1] = noise;
			imageData.data[i + 2] = noise;
			imageData.data[i + 3] = 255;
		}
		ctx.putImageData(imageData, 0, 0);
		animationId = requestAnimationFrame(drawNoise);
	}
	drawNoise();

	setTimeout(() => {
		cancelAnimationFrame(animationId);
		canvas.remove();
	}, duration);
}

/**
 * Apply pulse glow effect around page edges.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyPulseGlow(duration) {
	const style = document.createElement("style");
	style.id = "ghost-pulse-style";
	style.textContent = `
    @keyframes ghost-pulse {
      0%, 100% { box-shadow: inset 0 0 50px rgba(0, 255, 136, 0); }
      50% { box-shadow: inset 0 0 50px rgba(0, 255, 136, 0.3); }
    }
    body.ghost-pulse-active {
      animation: ghost-pulse 1s ease-in-out infinite;
    }
  `;
	document.head.appendChild(style);
	document.body.classList.add("ghost-pulse-active");

	setTimeout(() => {
		document.body.classList.remove("ghost-pulse-active");
		style.remove();
	}, duration);
}

/**
 * Apply screen flicker effect.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyFlicker(duration) {
	const style = document.createElement("style");
	style.id = "ghost-flicker-style";
	style.textContent = `
    @keyframes ghost-flicker {
      0%, 19%, 21%, 23%, 25%, 54%, 56%, 100% { opacity: 1; }
      20%, 24%, 55% { opacity: 0.7; }
    }
    body.ghost-flicker-active {
      animation: ghost-flicker 0.5s infinite;
    }
  `;
	document.head.appendChild(style);
	document.body.classList.add("ghost-flicker-active");

	setTimeout(() => {
		document.body.classList.remove("ghost-flicker-active");
		style.remove();
	}, duration);
}

/**
 * Highlight specific text on the page with glowing effect.
 * @param {string} searchText - Text to search and highlight
 * @returns {void}
 */
function highlightText(searchText) {
	if (!searchText) return;

	// Remove existing highlights
	document.querySelectorAll(".ghost-highlight").forEach((el) => {
		el.outerHTML = el.textContent || "";
	});

	// Create TreeWalker to find text nodes
	const walker = document.createTreeWalker(
		document.body,
		NodeFilter.SHOW_TEXT,
		null
	);

	/** @type {Text[]} */
	const nodesToHighlight = [];
	/** @type {Text|null} */
	let node;
	while ((node = /** @type {Text} */ (walker.nextNode()))) {
		if (
			node.textContent?.toLowerCase().includes(searchText.toLowerCase())
		) {
			nodesToHighlight.push(node);
		}
	}

	// Highlight found text
	nodesToHighlight.forEach((textNode) => {
		const text = textNode.textContent || "";
		const lowerText = text.toLowerCase();
		const lowerSearch = searchText.toLowerCase();
		const index = lowerText.indexOf(lowerSearch);

		if (index >= 0) {
			const before = text.substring(0, index);
			const match = text.substring(index, index + searchText.length);
			const after = text.substring(index + searchText.length);

			const span = document.createElement("span");
			span.className = "ghost-highlight";
			span.style.cssText = `
        background: linear-gradient(90deg, transparent, rgba(0, 255, 136, 0.5), transparent);
        padding: 2px;
        border-radius: 2px;
        animation: ghost-highlight-pulse 1s ease-in-out infinite;
      `;
			span.textContent = match;

			const fragment = document.createDocumentFragment();
			if (before) fragment.appendChild(document.createTextNode(before));
			fragment.appendChild(span);
			if (after) fragment.appendChild(document.createTextNode(after));

			textNode.parentNode?.replaceChild(fragment, textNode);
		}
	});

	// Add highlight animation style
	if (!document.getElementById("ghost-highlight-style")) {
		const style = document.createElement("style");
		style.id = "ghost-highlight-style";
		style.textContent = `
      @keyframes ghost-highlight-pulse {
        0%, 100% { box-shadow: 0 0 5px rgba(0, 255, 136, 0.5); }
        50% { box-shadow: 0 0 15px rgba(0, 255, 136, 0.8); }
      }
    `;
		document.head.appendChild(style);
	}
}

/**
 * Get page content for analysis.
 * Extracts visible text content, title, and URL.
 * @returns {PageContent} Page content object
 */
function getPageContent() {
	// Get visible text content (first 5000 chars)
	const bodyText = document.body.innerText
		.replace(/\s+/g, " ")
		.trim()
		.substring(0, 5000);

	return {
		bodyText,
		title: document.title,
		url: window.location.href,
	};
}

// Listen for messages from background script
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
	switch (message.type) {
		case "effect":
			applyEffect(message.effect, message.duration || 1000);
			sendResponse({ success: true });
			break;

		case "highlight":
			highlightText(message.text);
			sendResponse({ success: true });
			break;

		case "get_content":
			const content = getPageContent();
			sendResponse(content);
			break;

		default:
			sendResponse({ error: "Unknown message type" });
	}
	return false; // Synchronous response
});

log("Content script loaded on:", window.location.href);
