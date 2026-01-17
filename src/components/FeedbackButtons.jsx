/**
 * @fileoverview Feedback buttons for HITL (Human-in-the-Loop) pattern.
 * Allows users to rate ghost dialogue and report when stuck.
 * Implements Chapter 13 of Agentic Design Patterns.
 * @module FeedbackButtons
 */

import React, { useState, useCallback, useMemo, useEffect } from "react";
import PropTypes from "prop-types";

/**
 * Prevent event propagation for drag handling.
 * @param {React.SyntheticEvent} e - Event to stop
 */
const stopPropagation = (e) => e.stopPropagation();

/**
 * Feedback buttons component for rating dialogue.
 * Shows thumbs up/down buttons after dialogue is displayed.
 *
 * @param {Object} props - Component props
 * @param {string} props.content - The dialogue content being rated
 * @param {function} props.onFeedback - Callback for feedback submission (target, content, isPositive, comment?)
 * @param {boolean} [props.disabled=false] - Whether buttons are disabled
 * @returns {JSX.Element} Feedback buttons element
 */
export const DialogueFeedback = React.memo(
	({ content, onFeedback, disabled = false }) => {
		const [submitted, setSubmitted] = useState(null); // 'positive' | 'negative' | null

		const handlePositive = useCallback(() => {
			if (disabled || submitted) return;
			setSubmitted("positive");
			onFeedback?.("dialogue", content, true);
		}, [content, onFeedback, disabled, submitted]);

		const handleNegative = useCallback(() => {
			if (disabled || submitted) return;
			setSubmitted("negative");
			onFeedback?.("dialogue", content, false);
		}, [content, onFeedback, disabled, submitted]);

		// Reset when content changes
		useEffect(() => {
			setSubmitted(null);
		}, [content]);

		if (!content) return null;

		return (
			<div
				className="feedback-buttons"
				role="group"
				aria-label="Rate this response"
			>
				<button
					type="button"
					className={`feedback-btn positive ${submitted === "positive" ? "selected" : ""}`}
					onClick={handlePositive}
					onMouseDown={stopPropagation}
					disabled={disabled || submitted !== null}
					aria-label="Good response"
					aria-pressed={submitted === "positive"}
					title="This was helpful"
				>
					<span aria-hidden="true">👍</span>
				</button>
				<button
					type="button"
					className={`feedback-btn negative ${submitted === "negative" ? "selected" : ""}`}
					onClick={handleNegative}
					onMouseDown={stopPropagation}
					disabled={disabled || submitted !== null}
					aria-label="Poor response"
					aria-pressed={submitted === "negative"}
					title="Not helpful"
				>
					<span aria-hidden="true">👎</span>
				</button>
				{submitted && (
					<span className="feedback-thanks" aria-live="polite">
						Thanks!
					</span>
				)}
			</div>
		);
	}
);

DialogueFeedback.displayName = "DialogueFeedback";

DialogueFeedback.propTypes = {
	content: PropTypes.string,
	onFeedback: PropTypes.func.isRequired,
	disabled: PropTypes.bool,
};

DialogueFeedback.defaultProps = {
	content: "",
	disabled: false,
};

/**
 * "I'm Stuck" button component.
 * Triggers escalation when user needs extra help.
 *
 * @param {Object} props - Component props
 * @param {function} props.onStuck - Callback when user clicks (timeStuckSecs, description?)
 * @param {number} props.puzzleStartTime - Timestamp when puzzle started (for calculating time stuck)
 * @param {boolean} [props.disabled=false] - Whether button is disabled
 * @returns {JSX.Element} Stuck button element
 */
export const StuckButton = React.memo(
	({ onStuck, puzzleStartTime, disabled = false }) => {
		const [showInput, setShowInput] = useState(false);
		const [description, setDescription] = useState("");
		const [submitted, setSubmitted] = useState(false);

		const handleClick = useCallback(() => {
			if (disabled || submitted) return;
			setShowInput(true);
		}, [disabled, submitted]);

		const handleSubmit = useCallback(() => {
			const timeStuckSecs = puzzleStartTime
				? Math.floor((Date.now() - puzzleStartTime) / 1000)
				: 0;

			onStuck?.(timeStuckSecs, description || null);
			setSubmitted(true);
			setShowInput(false);
			setDescription("");
		}, [onStuck, puzzleStartTime, description]);

		const handleCancel = useCallback(() => {
			setShowInput(false);
			setDescription("");
		}, []);

		const handleInputChange = useCallback((e) => {
			setDescription(e.target.value);
		}, []);

		// Reset when puzzle changes (puzzleStartTime changes)
		useEffect(() => {
			setSubmitted(false);
			setShowInput(false);
			setDescription("");
		}, [puzzleStartTime]);

		if (submitted) {
			return (
				<div className="stuck-submitted" aria-live="polite">
					<span aria-hidden="true">🤝</span> Help is on the way...
				</div>
			);
		}

		if (showInput) {
			return (
				<div
					className="stuck-input-wrapper"
					role="dialog"
					aria-label="Describe your difficulty"
				>
					<textarea
						className="stuck-description"
						placeholder="What's confusing? (optional)"
						value={description}
						onChange={handleInputChange}
						onMouseDown={stopPropagation}
						rows={2}
						maxLength={200}
						aria-label="Description of difficulty"
					/>
					<div className="stuck-input-actions">
						<button
							type="button"
							className="stuck-submit-btn"
							onClick={handleSubmit}
							onMouseDown={stopPropagation}
						>
							Send
						</button>
						<button
							type="button"
							className="stuck-cancel-btn"
							onClick={handleCancel}
							onMouseDown={stopPropagation}
						>
							Cancel
						</button>
					</div>
				</div>
			);
		}

		return (
			<button
				type="button"
				className="stuck-btn"
				onClick={handleClick}
				onMouseDown={stopPropagation}
				disabled={disabled}
				aria-label="I'm stuck and need help"
				title="Request extra help"
			>
				<span aria-hidden="true">🆘</span> I'm Stuck
			</button>
		);
	}
);

StuckButton.displayName = "StuckButton";

StuckButton.propTypes = {
	onStuck: PropTypes.func.isRequired,
	puzzleStartTime: PropTypes.number,
	disabled: PropTypes.bool,
};

StuckButton.defaultProps = {
	puzzleStartTime: null,
	disabled: false,
};

/**
 * Settings toggle for intelligent mode features.
 *
 * @param {Object} props - Component props
 * @param {Object} props.settings - Current settings { intelligent_mode, reflection, guardrails }
 * @param {function} props.onToggleIntelligent - Toggle intelligent mode
 * @param {function} props.onToggleReflection - Toggle reflection mode
 * @param {function} props.onToggleGuardrails - Toggle guardrails mode
 * @param {boolean} [props.disabled=false] - Whether toggles are disabled
 * @returns {JSX.Element} Settings panel element
 */
export const IntelligentModeSettings = React.memo(
	({
		settings,
		onToggleIntelligent,
		onToggleReflection,
		onToggleGuardrails,
		disabled = false,
	}) => {
		const [expanded, setExpanded] = useState(false);

		const toggleExpanded = useCallback(() => {
			setExpanded((prev) => !prev);
		}, []);

		const handleIntelligent = useCallback(() => {
			onToggleIntelligent?.(!settings?.intelligent_mode);
		}, [onToggleIntelligent, settings?.intelligent_mode]);

		const handleReflection = useCallback(() => {
			onToggleReflection?.(!settings?.reflection);
		}, [onToggleReflection, settings?.reflection]);

		const handleGuardrails = useCallback(() => {
			onToggleGuardrails?.(!settings?.guardrails);
		}, [onToggleGuardrails, settings?.guardrails]);

		// Count enabled features for badge
		const enabledCount = useMemo(() => {
			if (!settings) return 0;
			return (
				(settings.intelligent_mode ? 1 : 0) +
				(settings.reflection ? 1 : 0) +
				(settings.guardrails ? 1 : 0)
			);
		}, [settings]);

		return (
			<div className="intelligent-settings">
				<button
					type="button"
					className={`settings-toggle-btn ${expanded ? "expanded" : ""}`}
					onClick={toggleExpanded}
					onMouseDown={stopPropagation}
					aria-expanded={expanded}
					aria-controls="intelligent-settings-panel"
				>
					<span aria-hidden="true">⚙️</span> Agent Settings
					<span className="settings-badge">{enabledCount}/3</span>
				</button>

				{expanded && (
					<div
						id="intelligent-settings-panel"
						className="settings-panel"
						role="group"
						aria-label="Intelligent mode settings"
					>
						<label className="setting-chip">
							<input
								type="checkbox"
								checked={settings?.intelligent_mode ?? true}
								onChange={handleIntelligent}
								onMouseDown={stopPropagation}
								disabled={disabled}
							/>
							<span className="chip-content">
								<span className="chip-icon" aria-hidden="true">
									🧠
								</span>
								Planning
							</span>
						</label>

						<label className="setting-chip">
							<input
								type="checkbox"
								checked={settings?.reflection ?? true}
								onChange={handleReflection}
								onMouseDown={stopPropagation}
								disabled={disabled}
							/>
							<span className="chip-content">
								<span className="chip-icon" aria-hidden="true">
									🪞
								</span>
								Reflection
							</span>
						</label>

						<label className="setting-chip">
							<input
								type="checkbox"
								checked={settings?.guardrails ?? true}
								onChange={handleGuardrails}
								onMouseDown={stopPropagation}
								disabled={disabled}
							/>
							<span className="chip-content">
								<span className="chip-icon" aria-hidden="true">
									🛡️
								</span>
								Safety
							</span>
						</label>
					</div>
				)}
			</div>
		);
	}
);

IntelligentModeSettings.displayName = "IntelligentModeSettings";

IntelligentModeSettings.propTypes = {
	settings: PropTypes.shape({
		intelligent_mode: PropTypes.bool,
		reflection: PropTypes.bool,
		guardrails: PropTypes.bool,
	}),
	onToggleIntelligent: PropTypes.func,
	onToggleReflection: PropTypes.func,
	onToggleGuardrails: PropTypes.func,
	disabled: PropTypes.bool,
};

IntelligentModeSettings.defaultProps = {
	settings: null,
	onToggleIntelligent: null,
	onToggleReflection: null,
	onToggleGuardrails: null,
	disabled: false,
};

/**
 * System Controls Accordion - collapsible panel for system buttons.
 *
 * @param {Object} props - Component props
 * @param {Function} props.onExtension - Extension button handler
 * @param {Function} props.onPrivacy - Privacy button handler
 * @param {Function} props.onChangeKey - Change Key button handler
 * @param {Function} props.onReset - Reset Game button handler
 * @returns {JSX.Element} Accordion with system control buttons
 */
export const SystemControlsAccordion = React.memo(
	({ onExtension, onPrivacy, onChangeKey, onReset }) => {
		const [expanded, setExpanded] = useState(false);

		const toggleExpanded = useCallback(() => {
			setExpanded((prev) => !prev);
		}, []);

		return (
			<div className="system-accordion">
				<button
					type="button"
					className={`system-accordion-toggle ${expanded ? "expanded" : ""}`}
					onClick={toggleExpanded}
					onMouseDown={stopPropagation}
					aria-expanded={expanded}
					aria-controls="system-accordion-panel"
				>
					<span aria-hidden="true">🔧</span> System Settings
					<span className="accordion-chevron" aria-hidden="true">
						{expanded ? "▲" : "▼"}
					</span>
				</button>

				{expanded && (
					<div
						id="system-accordion-panel"
						className="system-accordion-panel"
						role="group"
						aria-label="System settings"
					>
						<button
							type="button"
							className="system-accordion-btn"
							onMouseDown={stopPropagation}
							onClick={onExtension}
						>
							<span aria-hidden="true">🔌</span> Extension
						</button>

						<button
							type="button"
							className="system-accordion-btn"
							onMouseDown={stopPropagation}
							onClick={onPrivacy}
						>
							<span aria-hidden="true">🔒</span> Privacy
						</button>

						<button
							type="button"
							className="system-accordion-btn change-key"
							onMouseDown={stopPropagation}
							onClick={onChangeKey}
						>
							<span aria-hidden="true">🔑</span> Change Key
						</button>

						<button
							type="button"
							className="system-accordion-btn danger"
							onMouseDown={stopPropagation}
							onClick={onReset}
						>
							<span aria-hidden="true">🗑️</span> Reset Game
						</button>
					</div>
				)}
			</div>
		);
	}
);

SystemControlsAccordion.displayName = "SystemControlsAccordion";

SystemControlsAccordion.propTypes = {
	onExtension: PropTypes.func,
	onPrivacy: PropTypes.func,
	onChangeKey: PropTypes.func,
	onReset: PropTypes.func,
};

SystemControlsAccordion.defaultProps = {
	onExtension: null,
	onPrivacy: null,
	onChangeKey: null,
	onReset: null,
};
