/**
 * @fileoverview Ghost overlay (reflector-only) with status chips and dialogue.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { safeInvoke } from "../utils/data";
import { useGhostGame } from "../hooks/useTauriCommands";
import { useActionManagement } from "../hooks/useActionManagement";
import { DialogueFeedback, StuckButton } from "./FeedbackButtons";

const GHOST_SPRITES = Object.freeze({
  idle: `
    .-.
   (o.o)
    |=|
   __|__
  //.=|=.\\
 // .=|=. \\
 \\ .=|=. //
  \\(_=_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
  thinking: `
    .-.
   (?.?)
    |~|
   __|__
  //.~|~.\\
 // .~|~. \\
 \\ .~|~. //
  \\(_~_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
  searching: `
    .-.
   (>.<)
    |*|
   __|__
  //*=|=*\\
 // *=|=* \\
 \\ *=|=* //
  \\(_*_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
  celebrate: `
    \o/
   (^.^)
    |!|
   __|__
  //!=|=!\\
 // !=|=! \\
 \\ !=|=! //
  \\(_!_)//
   (:| |:)
    || ||
    () ()
    || ||
    || ||
   ==' '==
  `,
});

const TYPEWRITER_SPEED = 20;
const QUICK_ASK_PLACEHOLDER = "Ask a quick question...";

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
  const typewriterRef = useRef(null);
  const hasPromptedConsentRef = useRef(false);
  const [puzzleStartTime, setPuzzleStartTime] = useState(null);
  const [quickAsk, setQuickAsk] = useState({ prompt: "", response: "", error: "", isLoading: false, isOpen: false });

  const isCompanionMode = systemStatus?.currentMode === "companion";
  const readOnlyMode = !!privacySettings?.read_only_mode;
  const hasConsent =
    !!privacySettings?.capture_consent &&
    !!privacySettings?.ai_analysis_consent &&
    !!privacySettings?.privacy_notice_acknowledged &&
    !privacySettings?.read_only_mode;

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
  } = useActionManagement(privacySettings?.autonomy_level || "observer", !!systemStatus?.apiKeyConfigured);

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

  const extensionConnectedValue = extensionConnected || systemStatus?.extensionConnected;
  const keyConfigured = !!systemStatus?.apiKeyConfigured;

  const clueText = useMemo(() => {
    if (gameState.clue) return gameState.clue;
    if (gameState.puzzleId) return "Loading puzzle...";
    if (isCompanionMode && autonomySettings?.autoPuzzleFromCompanion) {
      if (readOnlyMode) return "Read-only mode active.";
      if (!hasConsent) return "Privacy consent needed.";
      if (extensionConnectedValue) return "Watching your browsing...";
      return "Watching via screenshots...";
    }
    if (extensionConnectedValue) return "Extension connected. Browse to begin.";
    return "Waiting for signal...";
  }, [
    gameState.clue,
    gameState.puzzleId,
    isCompanionMode,
    autonomySettings?.autoPuzzleFromCompanion,
    readOnlyMode,
    hasConsent,
    extensionConnectedValue,
  ]);

  const displayDialogue = useMemo(() => {
    if (!gameState.dialogue) return clueText;
    if (gameState.dialogue.includes("Waiting for signal") && !gameState.puzzleId) {
      return clueText;
    }
    return gameState.dialogue;
  }, [gameState.dialogue, gameState.puzzleId, clueText]);

  useEffect(() => {
    if (typewriterRef.current) {
      clearInterval(typewriterRef.current);
    }
    const text = quickAsk.response || displayDialogue;
    let index = 0;
    setTypedDialogue("");
    typewriterRef.current = setInterval(() => {
      index += 1;
      setTypedDialogue(text.slice(0, index));
      if (index >= text.length) {
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
  }, [displayDialogue, quickAsk.response]);

  useEffect(() => {
    if (gameState.puzzleId) {
      setPuzzleStartTime(Date.now());
    } else {
      setPuzzleStartTime(null);
    }
  }, [gameState.puzzleId]);

  const openSettings = useCallback((section) => {
    invoke("open_settings", { section });
  }, []);

  const handleDrag = useCallback(async (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const interactive = target.closest("button, a, input, textarea, select");
    if (interactive) return;
    await safeInvoke("start_window_drag");
  }, []);

  const handleToggleMode = useCallback(() => {
    if (!setAppMode) return;
    setAppMode(isCompanionMode ? "game" : "companion");
  }, [isCompanionMode, setAppMode]);

  const handleToggleAuto = useCallback(() => {
    if (!setAutonomySettings) return;
    setAutonomySettings((prev) => ({
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
    if (isLoading || !systemStatus?.apiKeyConfigured) return;
    await captureAndAnalyze?.();
  }, [hasConsent, captureAndAnalyze, openSettings, isLoading, systemStatus?.apiKeyConfigured]);

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

  const handleQuickAskSubmit = useCallback(async (event) => {
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
  }, [quickAsk.prompt]);

  const glowIntensity = useMemo(() => {
    const base = 6;
    const multiplier = 18;
    return Math.min(base + gameState.proximity * multiplier, 28);
  }, [gameState.proximity]);

  const spriteStyle = useMemo(
    () => ({
      "--glow-intensity": `${glowIntensity}px`,
    }),
    [glowIntensity]
  );

  useEffect(() => {
    if (!quickAsk.response) return;
    const id = setTimeout(() => {
      setQuickAsk((prev) => ({ ...prev, response: "" }));
    }, 12000);
    return () => clearTimeout(id);
  }, [quickAsk.response]);

  return (
    <div
      className={`ghost-container ${readOnlyMode ? "read-only-mode" : ""} ${
        isCompanionMode ? "companion-mode" : "game-mode"
      }`}
      onMouseDown={handleDrag}
    >
      <div className="ghost-header">
        <div className="toggle-row">
          <button
            type="button"
            className={`toggle-chip ${isCompanionMode ? "active" : ""}`}
            onClick={handleToggleMode}
            aria-pressed={isCompanionMode}
          >
            {isCompanionMode ? "Companion" : "Game"}
          </button>
          {isCompanionMode && (
            <button
              type="button"
              className={`toggle-chip ${autonomySettings?.autoPuzzleFromCompanion ? "active" : ""}`}
              onClick={handleToggleAuto}
              aria-pressed={!!autonomySettings?.autoPuzzleFromCompanion}
            >
              Auto
            </button>
          )}
          <button
            type="button"
            className={`toggle-chip ${readOnlyMode ? "active" : ""}`}
            onClick={handleToggleReadOnly}
            aria-pressed={readOnlyMode}
          >
            Read-only
          </button>
          <button type="button" className="toggle-chip settings" onClick={() => openSettings("general")}>
            Settings
          </button>
        </div>
        {!hasConsent && (
          <button type="button" className="ghost-alert" onClick={() => openSettings("privacy")}>
            Consent required — open Privacy
          </button>
        )}
        {!keyConfigured && (
          <button type="button" className="ghost-alert" onClick={() => openSettings("keys")}>
            API key missing — open Keys
          </button>
        )}
        {keyConfigured && !extensionConnectedValue && (
          <button type="button" className="ghost-alert" onClick={() => openSettings("extensions")}>
            Extension disconnected — open Extensions
          </button>
        )}
      </div>

      <div className="ghost-body compact">
        <button
          type="button"
          className={`ghost-sprite state-${gameState.state}`}
          onClick={handleToggleMode}
          aria-label="Toggle mode"
          style={spriteStyle}
        >
          <pre className="ascii-art" aria-hidden="true">
            {GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle}
          </pre>
        </button>
        <div className="dialogue-box compact" role="status" aria-live="polite">
          <div className="dialogue-top">
            <div className="proximity-bar">
              <div className="proximity-fill" style={{ width: `${Math.min(100, Math.max(0, gameState.proximity * 100))}%` }} />
            </div>
          </div>
          <div className="dialogue-text">{typedDialogue}</div>
          {isLoading && <span className="dialogue-loading">Analyzing…</span>}
          {companionBehavior?.suggestion && (
            <div className="companion-line">{companionBehavior.suggestion}</div>
          )}
          <div className="dialogue-actions">
            {gameState.state === "idle" && !gameState.puzzleId && keyConfigured && (
              <>
                <button type="button" className="mini-btn" onClick={handleAnalyze}>
                  Analyze
                </button>
                <button type="button" className="mini-btn" onClick={handleCreatePuzzle}>
                  {isCompanionMode ? "Create" : "Start"}
                </button>
              </>
            )}
            {gameState.state === "idle" && gameState.puzzleId && keyConfigured && (
              <>
                <button type="button" className="mini-btn" onClick={showHint} disabled={!gameState.hintAvailable}>
                  Hint
                </button>
                <button type="button" className="mini-btn" onClick={handleVerify}>
                  Verify
                </button>
              </>
            )}
            {keyConfigured && (
              <button type="button" className="mini-btn" onClick={() => setQuickAsk((prev) => ({ ...prev, isOpen: !prev.isOpen }))}>
                Ask
              </button>
            )}
          </div>
          {quickAsk.isOpen && (
            <form className="quick-ask-row" onSubmit={handleQuickAskSubmit}>
              <input
                className="quick-ask-input"
                value={quickAsk.prompt}
                onChange={(event) => setQuickAsk((prev) => ({ ...prev, prompt: event.target.value }))}
                placeholder={QUICK_ASK_PLACEHOLDER}
                aria-label="Quick ask"
              />
              <button type="submit" className="mini-btn" disabled={quickAsk.isLoading}>Send</button>
            </form>
          )}
          {quickAsk.error && <div className="dialogue-error">{quickAsk.error}</div>}
          {quickAsk.response && (
            <button type="button" className="mini-btn subtle" onClick={() => setQuickAsk({ prompt: "", response: "", error: "", isLoading: false, isOpen: false })}>
              Clear response
            </button>
          )}
          <div className="feedback-row">
            <DialogueFeedback content={displayDialogue} onFeedback={submitFeedback} disabled={isLoading} />
            {gameState.puzzleId && (
              <StuckButton onStuck={handleStuck} puzzleStartTime={puzzleStartTime} disabled={isLoading} />
            )}
            {(rollbackStatus?.can_undo || rollbackStatus?.can_redo) && (
              <div className="rollback-row">
                <button type="button" className="mini-btn" onClick={undoAction} disabled={!rollbackStatus?.can_undo}>Undo</button>
                <button type="button" className="mini-btn" onClick={redoAction} disabled={!rollbackStatus?.can_redo}>Redo</button>
              </div>
            )}
          </div>
        </div>
      </div>

      {pendingActions.length > 0 && (
        <div className="pending-mini" role="alertdialog" aria-label="Pending action">
          <div className="pending-title">Action needs approval</div>
          <div className="pending-desc">{pendingActions[0].description}</div>
          <div className="pending-actions">
            <button type="button" className="mini-btn" onClick={() => approveAction(pendingActions[0].id)}>
              Allow
            </button>
            <button type="button" className="mini-btn subtle" onClick={() => denyAction(pendingActions[0].id)}>
              Deny
            </button>
          </div>
        </div>
      )}

      {actionPreview && (
        <div className="preview-mini" role="alertdialog" aria-label="Action preview">
          <div className="pending-title">Preview: {actionPreview.action?.action_type}</div>
          <div className="pending-desc">{actionPreview.action?.description}</div>
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
              {actionPreview.visual_preview.preview_type === "url_card" && (
                <div className="preview-pill">{actionPreview.visual_preview.content}</div>
              )}
              {actionPreview.visual_preview.preview_type === "text_selection" && (
                <div className="preview-pill">{actionPreview.visual_preview.content}</div>
              )}
            </div>
          )}
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
                        const normalized = param.param_type === "number" || param.param_type === "duration" ? Number(nextValue) : nextValue;
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
            <button type="button" className="mini-btn" onClick={approvePreview}>Execute</button>
            <button type="button" className="mini-btn subtle" onClick={() => denyPreview("User denied")}>Cancel</button>
          </div>
        </div>
      )}
    </div>
  );
};

export default Ghost;
