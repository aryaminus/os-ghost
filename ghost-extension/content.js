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

// ============================================================================
// PII Sanitization (Defense in Depth - matches backend privacy.rs)
// ============================================================================

/**
 * Regex patterns for PII detection (compiled once for performance)
 * @type {Object.<string, RegExp>}
 */
const PII_PATTERNS = {
	email: /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/gi,
	phone: /(\+\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}/g,
	creditCard: /\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b/g,
	ssn: /\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b/g,
	// Common API key patterns
	apiKey: /\b(?:sk_live_|ghp_|gho_|glpat-|xoxb-|xoxp-|AKIA|AIza)[a-zA-Z0-9_\-]{20,}\b/gi,
};

/**
 * Sanitize text by redacting PII patterns.
 * This provides defense-in-depth before data leaves the browser.
 * @param {string} text - Text to sanitize
 * @returns {string} Sanitized text with PII replaced
 */
function sanitizePII(text) {
	if (!text) return text;
	
	let sanitized = text;
	sanitized = sanitized.replace(PII_PATTERNS.email, "[REDACTED_EMAIL]");
	sanitized = sanitized.replace(PII_PATTERNS.phone, "[REDACTED_PHONE]");
	sanitized = sanitized.replace(PII_PATTERNS.creditCard, "[REDACTED_CARD]");
	sanitized = sanitized.replace(PII_PATTERNS.ssn, "[REDACTED_SSN]");
	sanitized = sanitized.replace(PII_PATTERNS.apiKey, "[REDACTED_API_KEY]");
	
	return sanitized;
}


/**
 * Apply visual effect to the page.
 * @param {EffectType} effect - Effect type to apply
 * @param {number} duration - Effect duration in milliseconds
 * @returns {void}
 */
function applyEffect(effect, duration) {
	if (!effect) return;

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

}

/**
 * Apply glitch distortion effect to page.
 * Creates clip-path animation that distorts the view.
 * @param {number} duration - Effect duration in ms
 * @returns {void}
 */
function applyGlitchEffect(duration) {
	document.getElementById("ghost-glitch-style")?.remove();
	document.body.classList.remove("ghost-glitch-active");

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
	document.getElementById("ghost-scanlines")?.remove();
	document.getElementById("ghost-scanlines-style")?.remove();

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
	document.getElementById("ghost-static")?.remove();

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
	if (!ctx) {
		canvas.remove();
		return;
	}

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
	document.getElementById("ghost-pulse-style")?.remove();
	document.body.classList.remove("ghost-pulse-active");

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
	document.getElementById("ghost-flicker-style")?.remove();
	document.body.classList.remove("ghost-flicker-active");

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
 * Ghost Trail effect - creates fading particles behind cursor.
 */
class GhostTrail {
	constructor() {
		this.active = false;
		this.particles = [];
		this.ctx = null;
		this.canvas = null;
		this.animationId = null;
		this.handleResize = null;

		this.handleMouseMove = this.handleMouseMove.bind(this);
		this.animate = this.animate.bind(this);
	}

	start() {
		if (this.active) return;
		this.active = true;

		this.canvas = document.createElement("canvas");
		this.canvas.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            pointer-events: none;
            z-index: 999998;
        `;
		document.body.appendChild(this.canvas);

		this.ctx = this.canvas.getContext("2d");
		this.resize();

		// Store bound resize handler for proper cleanup
		this.handleResize = () => this.resize();
		window.addEventListener("resize", this.handleResize);
		window.addEventListener("mousemove", this.handleMouseMove);

		this.animate();
	}

	stop() {
		if (!this.active) return;
		this.active = false;

		// Remove all event listeners to prevent memory leaks
		window.removeEventListener("mousemove", this.handleMouseMove);
		if (this.handleResize) {
			window.removeEventListener("resize", this.handleResize);
			this.handleResize = null;
		}
		cancelAnimationFrame(this.animationId);

		if (this.canvas) {
			this.canvas.remove();
			this.canvas = null;
		}
		this.ctx = null;
		this.particles = [];
	}

	resize() {
		if (this.canvas) {
			this.canvas.width = window.innerWidth;
			this.canvas.height = window.innerHeight;
		}
	}

	handleMouseMove(e) {
		this.particles.push({
			x: e.clientX,
			y: e.clientY,
			size: Math.random() * 5 + 2,
			life: 1.0,
			velocity: {
				x: (Math.random() - 0.5) * 2,
				y: (Math.random() - 0.5) * 2,
			},
		});
	}

	animate() {
		if (!this.active || !this.ctx || !this.canvas) return;

		this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

		// Filter dead particles in single pass - O(n) instead of O(n²) splice
		let writeIndex = 0;
		for (let i = 0; i < this.particles.length; i++) {
			const p = this.particles[i];
			p.life -= 0.02;
			p.x += p.velocity.x;
			p.y += p.velocity.y;

			if (p.life > 0) {
				// Draw live particle
				this.ctx.beginPath();
				this.ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
				this.ctx.fillStyle = `rgba(0, 255, 136, ${p.life * 0.5})`;
				this.ctx.fill();

				// Compact array in-place
				if (writeIndex !== i) {
					this.particles[writeIndex] = p;
				}
				writeIndex++;
			}
		}
		// Truncate dead particles
		this.particles.length = writeIndex;

		this.animationId = requestAnimationFrame(this.animate);
	}
}

const ghostTrail = new GhostTrail();

/**
 * Apply Portal Flash effect - transition effect for solving.
 * @param {number} duration - Effect duration in ms
 */
function applyPortalFlash(duration) {
	const overlay = document.createElement("div");
	overlay.style.cssText = `
        position: fixed;
        top: 0; 
        left: 0;
        width: 100%;
        height: 100%;
        background: radial-gradient(circle, transparent 0%, rgba(0, 255, 136, 0.2) 50%, rgba(0, 255, 136, 0.8) 100%);
        opacity: 0;
        transition: opacity 0.5s ease-out;
        z-index: 999999;
        pointer-events: none;
    `;
	document.body.appendChild(overlay);

	// Trigger animation
	requestAnimationFrame(() => {
		overlay.style.opacity = "1";
		setTimeout(() => {
			overlay.style.opacity = "0";
			setTimeout(() => overlay.remove(), 500);
		}, duration);
	});
}

/**
 * Get page content for analysis.
 * Extracts visible text content, title, and URL.
 * Applies PII sanitization before returning.
 * @returns {PageContent} Page content object with sanitized text
 */
function getPageContent() {
	// Get visible text content (first 5000 chars)
	const rawText = document.body.innerText
		.replace(/\s+/g, " ")
		.trim()
		.substring(0, 5000);

	// Sanitize PII before sending to native app (defense in depth)
	const bodyText = sanitizePII(rawText);

	return {
		bodyText,
		title: document.title,
		url: window.location.href,
	};
}

/**
 * Cached page content for deferred extraction
 * @type {PageContent|null}
 */
let cachedContent = null;

/**
 * Extract page content using requestIdleCallback for performance.
 * Defers heavy DOM operations to idle periods to avoid blocking page load.
 * @param {Function} callback - Called with extracted content
 */
function extractContentWhenIdle(callback) {
	// If requestIdleCallback not supported, fall back to setTimeout
	const scheduleIdle = window.requestIdleCallback || ((cb) => setTimeout(cb, 50));
	
	scheduleIdle(() => {
		cachedContent = getPageContent();
		callback(cachedContent);
	}, { timeout: 2000 }); // Max 2s wait
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

		case "start_trail":
			ghostTrail.start();
			sendResponse({ success: true });
			break;

		case "stop_trail":
			ghostTrail.stop();
			sendResponse({ success: true });
			break;

		case "flash":
			applyPortalFlash(message.duration || 1000);
			sendResponse({ success: true });
			break;

		case "get_content":
			// Return cached content if available, otherwise extract synchronously
			// (background needs immediate response for native messaging)
			const content = cachedContent || getPageContent();
			sendResponse(content);
			break;


		default:
			sendResponse({ error: "Unknown message type" });
	}
	return false; // Synchronous response
});

// Proactively extract content when idle after page load
// This pre-populates the cache for faster get_content responses
if (document.readyState === "complete") {
	extractContentWhenIdle(() => log("Content cached on load"));
} else {
	window.addEventListener("load", () => {
		extractContentWhenIdle(() => log("Content cached after load"));
	}, { once: true });
}

log("Content script loaded on:", window.location.href);
