/**
 * @fileoverview Ghost overlay - Clippy-inspired ambient assistant.
 * Minimal footprint, non-interruptive, speech bubble + sprite design.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { safeInvoke } from "../utils/data";
import { useGhostGame } from "../hooks/useTauriCommands";
import { useActionManagement } from "../hooks/useActionManagement";
import { DialogueFeedback, StuckButton } from "./FeedbackButtons";

// Compact ASCII sprites - smaller footprint for Clippy-like design
const GHOST_SPRITES = Object.freeze({
  idle: `
  .--.
 ( o.o)
  |> |
  _|__|_
 /_/==\\_\\
`,
  thinking: `
  .--.
 ( ?.? )
  |~~|
  _|__|_
 /_/~~\\_\\
`,
  searching: `
  .--.
 ( >.< )
  |**|
  _|__|_
 /_/**\\_\\
`,
  celebrate: `
  \\o/
 ( ^.^ )
  |!!|
  _|__|_
 /_/!!\\_\\
`,
});

const TYPEWRITER_SPEED = 18;

/**
 * Settings gear icon SVG
 */
const GearIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4" />
  </svg>
);

/**
 * Status dot indicator
 */
const StatusDot = ({ status, pulse = false }) => (
  <span 
    className={`status-dot-mini ${status} ${pulse ? "pulse" : ""}`} 
    aria-hidden="true" 
  />
);

/**
 * Toggle chip button
 */
const ToggleChip = ({ label, active, onClick, disabled, className = "" }) => (
  <button
    type="button"
    className={`toggle-chip ${active ? "active" : ""} ${className}`}
    onClick={onClick}
    disabled={disabled}
    aria-pressed={active}
  >
    {label}
  </button>
);

const Ghost = () => {
  const {
    gameState,
    isLoading,
    extensionConnected,
    systemStatus,
    companionBehavior,
    captureAndAnalyze,
    verifyScreenshotProof,
    showHint,
    triggerDynamicPuzzle,
    generateAdaptivePuzzle,
    submitFeedback,
    reportStuck,
    detectSystemStatus,
    setAppMode,
    autonomySettings,
    setAutonomySettings,
  } = useGhostGame();

  const [privacySettings, setPrivacySettings] = useState(null);
  const [typedDialogue, setTypedDialogue] = useState("");
  const lastTextRef = useRef("");
  const typewriterRef = useRef(null);
  const hasPromptedConsentRef = useRef(false);
  const [puzzleStartTime, setPuzzleStartTime] = useState(null);
  const [quickAsk, setQuickAsk] = useState({
    prompt: "",
    response: "",
    error: "",
    isLoading: false,
    isOpen: false,
  });

  // Derived state
  const isCompanionMode = systemStatus?.currentMode === "companion";
  const readOnlyMode = !!privacySettings?.read_only_mode;
  const hasConsent =
    !!privacySettings?.capture_consent &&
    !!privacySettings?.ai_analysis_consent &&
    !!privacySettings?.privacy_notice_acknowledged &&
    !privacySettings?.read_only_mode;
  const extensionConnectedValue = extensionConnected || systemStatus?.extensionConnected;
  const keyConfigured = !!systemStatus?.apiKeyConfigured;
  const autoMode = !!autonomySettings?.autoPuzzleFromCompanion;

  // Action management for approval flows
  const {
    pendingActions,
    actionPreview,
    rollbackStatus,
    approveAction,
    denyAction,
    undoAction,
    redoAction,
    approvePreview,
    denyPreview,
    editPreviewParam,
  } = useActionManagement(
    privacySettings?.autonomy_level || "observer",
    keyConfigured
  );

  // Load privacy settings
  const loadPrivacy = useCallback(async () => {
    const settings = await safeInvoke("get_privacy_settings", {}, null);
    if (settings) {
      setPrivacySettings(settings);
      if (!settings.privacy_notice_acknowledged && !hasPromptedConsentRef.current) {
        hasPromptedConsentRef.current = true;
        await invoke("open_settings", { section: "privacy" });
      }
    }
  }, []);

  useEffect(() => {
    loadPrivacy();
  }, [loadPrivacy]);

  // Listen for settings updates
  useEffect(() => {
    let unlisten = null;
    const setup = async () => {
      unlisten = await listen("settings:updated", () => {
        loadPrivacy();
        detectSystemStatus?.();
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [loadPrivacy, detectSystemStatus]);

  // Compute display text
  const displayText = useMemo(() => {
    // Priority: quick ask response > game dialogue > contextual clue
    if (quickAsk.response) return quickAsk.response;
    if (gameState.dialogue && !gameState.dialogue.includes("Waiting for signal")) {
      return gameState.dialogue;
    }
    if (gameState.clue) return gameState.clue;
    if (gameState.puzzleId) return "Loading puzzle...";

    // Contextual default messages based on state
    if (readOnlyMode) return "Read-only mode. I'm just watching.";
    if (!hasConsent) return "Grant consent to begin.";
    if (!keyConfigured) return "Configure your API key to start.";
    
    if (isCompanionMode) {
      if (autoMode) {
        return extensionConnectedValue
          ? "Auto mode: Watching your browsing..."
          : "Auto mode: Observing via screenshots...";
      }
      return extensionConnectedValue
        ? "Companion ready. Browse to begin."
        : "Observing via screenshots...";
    }
    return extensionConnectedValue
      ? "Game ready. Browse to begin."
      : "Waiting for signal...";
  }, [
    quickAsk.response,
    gameState.dialogue,
    gameState.clue,
    gameState.puzzleId,
    readOnlyMode,
    hasConsent,
    keyConfigured,
    isCompanionMode,
    autoMode,
    extensionConnectedValue,
  ]);

  // Typewriter effect - only triggers on actual text change
  useEffect(() => {
    if (typewriterRef.current) {
      clearInterval(typewriterRef.current);
    }
    if (displayText === lastTextRef.current) return;
    lastTextRef.current = displayText;

    let index = 0;
    setTypedDialogue("");
    typewriterRef.current = setInterval(() => {
      index += 1;
      setTypedDialogue(displayText.slice(0, index));
      if (index >= displayText.length) {
        clearInterval(typewriterRef.current);
        typewriterRef.current = null;
      }
    }, TYPEWRITER_SPEED);

    return () => {
      if (typewriterRef.current) {
        clearInterval(typewriterRef.current);
        typewriterRef.current = null;
      }
    };
  }, [displayText]);

  // Track puzzle timing
  useEffect(() => {
    setPuzzleStartTime(gameState.puzzleId ? Date.now() : null);
  }, [gameState.puzzleId]);

  // Clear quick ask response after delay
  useEffect(() => {
    if (!quickAsk.response) return;
    const id = setTimeout(() => {
      setQuickAsk((prev) => ({ ...prev, response: "" }));
    }, 15000);
    return () => clearTimeout(id);
  }, [quickAsk.response]);

  // Handlers
  const openSettings = useCallback((section = "general") => {
    invoke("open_settings", { section });
  }, []);

  const handleDrag = useCallback(async (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.closest("button, a, input, textarea, select")) return;
    await safeInvoke("start_window_drag");
  }, []);

  const handleToggleMode = useCallback(() => {
    setAppMode?.(isCompanionMode ? "game" : "companion");
  }, [isCompanionMode, setAppMode]);

  const handleToggleAuto = useCallback(() => {
    setAutonomySettings?.((prev) => ({
      ...prev,
      autoPuzzleFromCompanion: !prev.autoPuzzleFromCompanion,
    }));
  }, [setAutonomySettings]);

  const handleToggleReadOnly = useCallback(async () => {
    if (!privacySettings) return;
    const updated = await safeInvoke("update_privacy_settings", {
      captureConsent: privacySettings.capture_consent,
      aiAnalysisConsent: privacySettings.ai_analysis_consent,
      privacyNoticeAcknowledged: privacySettings.privacy_notice_acknowledged,
      readOnlyMode: !privacySettings.read_only_mode,
      autonomyLevel: privacySettings.autonomy_level || "autonomous",
      redactPii: privacySettings.redact_pii !== false,
    }, null);
    if (updated) setPrivacySettings(updated);
  }, [privacySettings]);

  const handleAnalyze = useCallback(async () => {
    if (!hasConsent) {
      openSettings("privacy");
      return;
    }
    if (isLoading || !keyConfigured) return;
    await captureAndAnalyze?.();
  }, [hasConsent, captureAndAnalyze, openSettings, isLoading, keyConfigured]);

  const handleVerify = useCallback(async () => {
    if (!hasConsent) {
      openSettings("privacy");
      return;
    }
    if (!gameState.puzzleId || isLoading) return;
    await verifyScreenshotProof?.();
  }, [hasConsent, verifyScreenshotProof, openSettings, gameState.puzzleId, isLoading]);

  const handleCreatePuzzle = useCallback(async () => {
    if (isCompanionMode) {
      await generateAdaptivePuzzle?.();
    } else {
      await triggerDynamicPuzzle?.();
    }
  }, [isCompanionMode, generateAdaptivePuzzle, triggerDynamicPuzzle]);

  const handleStuck = useCallback(
    async (timeStuckSecs, description) => {
      await reportStuck?.(timeStuckSecs, description);
      if (gameState.hintAvailable && !isLoading) {
        await showHint?.();
      }
    },
    [reportStuck, gameState.hintAvailable, isLoading, showHint]
  );

  const handleQuickAskSubmit = useCallback(
    async (event) => {
      event.preventDefault();
      if (!quickAsk.prompt.trim()) return;
      setQuickAsk((prev) => ({ ...prev, isLoading: true, error: "" }));
      try {
        const response = await invoke("quick_ask", { prompt: quickAsk.prompt.trim() });
        setQuickAsk({ prompt: "", response, error: "", isLoading: false, isOpen: false });
      } catch (err) {
        const message = typeof err === "string" ? err : err?.message || "Quick ask failed";
        setQuickAsk((prev) => ({ ...prev, isLoading: false, error: message }));
      }
    },
    [quickAsk.prompt]
  );

  // Compute system status for display
  const systemState = useMemo(() => {
    if (!keyConfigured) return { status: "error", label: "No API Key" };
    if (!hasConsent) return { status: "warning", label: "Consent needed" };
    if (readOnlyMode) return { status: "info", label: "Read-only" };
    if (!extensionConnectedValue) return { status: "warning", label: "No extension" };
    return { status: "ok", label: isCompanionMode ? "Companion" : "Game" };
  }, [keyConfigured, hasConsent, readOnlyMode, extensionConnectedValue, isCompanionMode]);

  // Compute primary action based on state
  const primaryAction = useMemo(() => {
    if (!keyConfigured) {
      return { label: "Setup", onClick: () => openSettings("keys") };
    }
    if (!hasConsent) {
      return { label: "Consent", onClick: () => openSettings("privacy") };
    }
    if (gameState.puzzleId) {
      return {
        label: "Hint",
        onClick: showHint,
        disabled: !gameState.hintAvailable || isLoading,
      };
    }
    return { label: "Analyze", onClick: handleAnalyze, disabled: isLoading };
  }, [keyConfigured, hasConsent, gameState.puzzleId, gameState.hintAvailable, isLoading, showHint, handleAnalyze, openSettings]);

  // Compute secondary action
  const secondaryAction = useMemo(() => {
    if (gameState.puzzleId) {
      return { label: "Verify", onClick: handleVerify, disabled: isLoading };
    }
    return {
      label: isCompanionMode ? "Create" : "Start",
      onClick: handleCreatePuzzle,
      disabled: isLoading || !hasConsent,
    };
  }, [gameState.puzzleId, isLoading, handleVerify, isCompanionMode, handleCreatePuzzle, hasConsent]);

  // Glow intensity based on proximity
  const glowIntensity = useMemo(() => {
    const base = 4;
    const multiplier = 12;
    return Math.min(base + gameState.proximity * multiplier, 20);
  }, [gameState.proximity]);

  // Mode class for theming - this drives CSS variable changes
  const modeClass = useMemo(() => {
    if (readOnlyMode) return "read-only-mode";
    return isCompanionMode ? "companion-mode" : "game-mode";
  }, [readOnlyMode, isCompanionMode]);

  return (
    <div
      className={`ghost-container ${modeClass}`}
      onMouseDown={handleDrag}
      style={{ "--glow-size": `${glowIntensity}px` }}
    >
      {/* Header: Toggle chips + Settings */}
      <header className="ghost-header">
        <div className="ghost-topbar">
          <div className="ghost-chips">
            <ToggleChip
              label={isCompanionMode ? "Companion" : "Game"}
              active={true}
              onClick={handleToggleMode}
            />
            {isCompanionMode && (
              <ToggleChip
                label="Auto"
                active={autoMode}
                onClick={handleToggleAuto}
              />
            )}
            <ToggleChip
              label="Read-only"
              active={readOnlyMode}
              onClick={handleToggleReadOnly}
              className="readonly"
            />
          </div>
          <button
            type="button"
            className="ghost-settings-btn"
            onClick={() => openSettings("general")}
            aria-label="Open settings"
            title="Settings"
          >
            <GearIcon />
          </button>
        </div>
        
        {/* Status alert bar - clickable to open relevant settings */}
        {(systemState.status !== "ok") && (
          <button
            type="button"
            className={`ghost-alert ${systemState.status}`}
            onClick={() => openSettings(
              !keyConfigured ? "keys" : 
              !hasConsent ? "privacy" : 
              readOnlyMode ? "privacy" :
              !extensionConnectedValue ? "extensions" : 
              "general"
            )}
          >
            <StatusDot status={systemState.status} pulse={isLoading} />
            <span>{systemState.label}</span>
          </button>
        )}
      </header>

      {/* Main content: Sprite + Bubble */}
      <main className="ghost-body">
        {/* ASCII Sprite */}
        <button
          type="button"
          className={`ghost-sprite state-${gameState.state}`}
          onClick={handleToggleMode}
          aria-label={`Toggle mode. Currently ${isCompanionMode ? "companion" : "game"} mode.`}
          title="Click to toggle mode"
        >
          <pre className="ascii-art" aria-hidden="true">
            {GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle}
          </pre>
        </button>

        {/* Speech Bubble */}
        <div className="dialogue-box" role="status" aria-live="polite">
          {/* Proximity indicator (mini bar) */}
          {gameState.proximity > 0 && (
            <div className="proximity-bar">
              <div
                className="proximity-fill"
                style={{ width: `${Math.min(100, gameState.proximity * 100)}%` }}
              />
            </div>
          )}

          {/* Dialogue text */}
          <div className="dialogue-text">
            {typedDialogue}
            {typedDialogue.length < displayText.length && (
              <span className="cursor" aria-hidden="true">|</span>
            )}
          </div>

          {/* Loading indicator */}
          {isLoading && <span className="dialogue-loading">Thinking...</span>}

          {/* Companion suggestion */}
          {companionBehavior?.suggestion && (
            <div className="companion-line">{companionBehavior.suggestion}</div>
          )}

          {/* Quick Ask error */}
          {quickAsk.error && (
            <div className="dialogue-error">{quickAsk.error}</div>
          )}

          {/* Action buttons */}
          <div className="dialogue-actions">
            <button
              type="button"
              className="mini-btn primary"
              onClick={primaryAction.onClick}
              disabled={primaryAction.disabled}
            >
              {primaryAction.label}
            </button>
            <button
              type="button"
              className="mini-btn"
              onClick={secondaryAction.onClick}
              disabled={secondaryAction.disabled}
            >
              {secondaryAction.label}
            </button>
            <button
              type="button"
              className="mini-btn"
              onClick={() => setQuickAsk((prev) => ({ ...prev, isOpen: !prev.isOpen }))}
              disabled={!keyConfigured}
              aria-expanded={quickAsk.isOpen}
            >
              Ask
            </button>
          </div>

          {/* Quick Ask input */}
          {quickAsk.isOpen && (
            <form className="quick-ask-row" onSubmit={handleQuickAskSubmit}>
              <input
                type="text"
                className="quick-ask-input"
                value={quickAsk.prompt}
                onChange={(e) => setQuickAsk((prev) => ({ ...prev, prompt: e.target.value }))}
                placeholder="Ask something..."
                aria-label="Quick question"
                autoFocus
              />
              <button
                type="submit"
                className="mini-btn"
                disabled={quickAsk.isLoading || !quickAsk.prompt.trim()}
              >
                {quickAsk.isLoading ? "..." : "Send"}
              </button>
            </form>
          )}

          {/* Clear quick ask response */}
          {quickAsk.response && (
            <button
              type="button"
              className="mini-btn subtle"
              onClick={() => setQuickAsk({ prompt: "", response: "", error: "", isLoading: false, isOpen: false })}
            >
              Clear response
            </button>
          )}

          {/* Feedback row - always visible when there's content */}
          <div className="feedback-row">
            <DialogueFeedback
              content={displayText}
              onFeedback={submitFeedback}
              disabled={isLoading}
            />
            {gameState.puzzleId && (
              <StuckButton
                onStuck={handleStuck}
                puzzleStartTime={puzzleStartTime}
                disabled={isLoading}
              />
            )}
          </div>

          {/* Undo/Redo controls */}
          {(rollbackStatus?.can_undo || rollbackStatus?.can_redo) && (
            <div className="rollback-row">
              <button
                type="button"
                className="mini-btn"
                onClick={undoAction}
                disabled={!rollbackStatus?.can_undo}
                title="Undo last action"
              >
                Undo
              </button>
              <button
                type="button"
                className="mini-btn"
                onClick={redoAction}
                disabled={!rollbackStatus?.can_redo}
                title="Redo action"
              >
                Redo
              </button>
            </div>
          )}
        </div>
      </main>

      {/* Pending Action Approval (overlay) */}
      {pendingActions.length > 0 && (
        <div className="pending-mini" role="alertdialog" aria-label="Pending action">
          <div className="pending-title">Action needs approval</div>
          <div className="pending-desc">{pendingActions[0].description}</div>
          <div className="pending-actions">
            <button
              type="button"
              className="mini-btn approve"
              onClick={() => approveAction(pendingActions[0].id)}
            >
              Allow
            </button>
            <button
              type="button"
              className="mini-btn deny"
              onClick={() => denyAction(pendingActions[0].id)}
            >
              Deny
            </button>
          </div>
        </div>
      )}

      {/* Action Preview (overlay) */}
      {actionPreview && (
        <div className="preview-mini" role="alertdialog" aria-label="Action preview">
          <div className="pending-title">Preview: {actionPreview.action?.action_type}</div>
          <div className="pending-desc">{actionPreview.action?.description}</div>

          {/* Visual preview */}
          {actionPreview.visual_preview && (
            <div className="preview-visual">
              {actionPreview.visual_preview.preview_type === "screenshot" && (
                <img
                  src={
                    actionPreview.visual_preview.content.startsWith("data:")
                      ? actionPreview.visual_preview.content
                      : `data:image/png;base64,${actionPreview.visual_preview.content}`
                  }
                  alt={actionPreview.visual_preview.alt_text || "Preview"}
                />
              )}
              {(actionPreview.visual_preview.preview_type === "url_card" ||
                actionPreview.visual_preview.preview_type === "text_selection") && (
                <div className="preview-pill">
                  {actionPreview.visual_preview.content}
                </div>
              )}
            </div>
          )}

          {/* Editable params */}
          {actionPreview.editable_params && Object.keys(actionPreview.editable_params).length > 0 && (
            <div className="preview-params">
              {Object.entries(actionPreview.editable_params).map(([name, param]) => (
                <div key={name} className="param-row">
                  <span className="param-label">{param.label}</span>
                  <button
                    type="button"
                    className="mini-btn subtle"
                    onClick={() => {
                      const nextValue = prompt(param.label, param.value);
                      if (nextValue !== null) {
                        const normalized =
                          param.param_type === "number" || param.param_type === "duration"
                            ? Number(nextValue)
                            : nextValue;
                        editPreviewParam(name, normalized);
                      }
                    }}
                  >
                    Edit
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="pending-actions">
            <button type="button" className="mini-btn approve" onClick={approvePreview}>
              Execute
            </button>
            <button
              type="button"
              className="mini-btn deny"
              onClick={() => denyPreview("User cancelled")}
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default Ghost;
