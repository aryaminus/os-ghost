/**
 * @fileoverview Feedback buttons for HITL (Human-in-the-Loop) pattern.
 * Allows users to rate ghost dialogue and report when stuck.
 * @module FeedbackButtons
 */

import React, { useState, useCallback, useEffect } from "react";
import PropTypes from "prop-types";

/** Prevent event propagation for drag handling */
const stopPropagation = (e) => e.stopPropagation();

/**
 * Feedback buttons component for rating dialogue.
 * Shows thumbs up/down buttons after dialogue is displayed.
 *
 * @param {Object} props - Component props
 * @param {string} props.content - The dialogue content being rated
 * @param {function} props.onFeedback - Callback for feedback submission
 * @param {boolean} [props.disabled=false] - Whether buttons are disabled
 * @returns {JSX.Element|null} Feedback buttons element
 */
export const DialogueFeedback = React.memo(
	({ content, onFeedback, disabled = false }) => {
		const [submitted, setSubmitted] = useState(null);

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

		useEffect(() => {
			setSubmitted(null);
		}, [content]);

		if (!content) return null;

		return (
			<div className="feedback-buttons" role="group" aria-label="Rate this response">
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
 * @param {function} props.onStuck - Callback when user clicks
 * @param {number} props.puzzleStartTime - Timestamp when puzzle started
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
				<div className="stuck-input-wrapper" role="dialog" aria-label="Describe your difficulty">
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
