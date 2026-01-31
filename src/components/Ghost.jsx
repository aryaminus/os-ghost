/**
 * @fileoverview Ghost overlay (reflector-only) with status chips and dialogue.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { safeInvoke } from "../utils/data";
import { useGhostGame } from "../hooks/useTauriCommands";
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

const Ghost = () => {
  const {
    gameState,
    isLoading,
    extensionConnected,
    systemStatus,
    submitFeedback,
    reportStuck,
    detectSystemStatus,
  } = useGhostGame();
  const [privacySettings, setPrivacySettings] = useState(null);
  const [typedDialogue, setTypedDialogue] = useState("");
  const typewriterRef = useRef(null);
  const hasPromptedConsentRef = useRef(false);
  const [puzzleStartTime, setPuzzleStartTime] = useState(null);

  const isCompanionMode = systemStatus?.currentMode === "companion";
  const readOnlyMode = !!privacySettings?.read_only_mode;

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

  useEffect(() => {
    if (typewriterRef.current) {
      clearInterval(typewriterRef.current);
    }
    const text = gameState.dialogue || "";
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
  }, [gameState.dialogue]);

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

  const consentStatus = useMemo(() => {
    if (!privacySettings) return "Checking…";
    if (privacySettings.read_only_mode) return "Read-only";
    if (
      privacySettings.capture_consent &&
      privacySettings.ai_analysis_consent &&
      privacySettings.privacy_notice_acknowledged
    ) {
      return "Granted";
    }
    return "Required";
  }, [privacySettings]);

  const autonomyLabel = privacySettings?.autonomy_level || "observer";
  const modeLabel = systemStatus?.currentMode || "companion";
  const extensionLabel =
    extensionConnected || systemStatus?.extensionConnected ? "Connected" : "Disconnected";
  const keyLabel = systemStatus?.apiKeyConfigured ? "Configured" : "Missing";

  return (
    <div
      className={`ghost-container ${readOnlyMode ? "read-only-mode" : ""} ${
        isCompanionMode ? "companion-mode" : "game-mode"
      }`}
      onMouseDown={handleDrag}
    >
      <div className="ghost-header">
        <div className="ghost-header-row">
          <div className="ghost-header-title">
            System Status
            <span className="ghost-header-subtitle">Tap any tile to open Settings</span>
          </div>
          <button
            type="button"
            className="ghost-settings-btn"
            onClick={() => openSettings("general")}
          >
            Open System Settings
          </button>
        </div>
        <div className="status-grid">
          <button type="button" className="status-card" onClick={() => openSettings("privacy")}>
            <span className="status-label">Consent</span>
            <span className="status-value">{consentStatus}</span>
          </button>
          <button type="button" className="status-card" onClick={() => openSettings("autonomy")}>
            <span className="status-label">Autonomy</span>
            <span className="status-value">{autonomyLabel}</span>
          </button>
          <button type="button" className="status-card" onClick={() => openSettings("extensions")}>
            <span className="status-label">Extension</span>
            <span className="status-value">{extensionLabel}</span>
          </button>
          <button type="button" className="status-card" onClick={() => openSettings("keys")}
          >
            <span className="status-label">Keys</span>
            <span className="status-value">{keyLabel}</span>
          </button>
          <button type="button" className="status-card" onClick={() => openSettings("general")}>
            <span className="status-label">Mode</span>
            <span className="status-value">{modeLabel}</span>
          </button>
        </div>
      </div>

      <div className="ghost-body">
        <pre className={`ghost-sprite ${gameState.state}`}>{GHOST_SPRITES[gameState.state] || GHOST_SPRITES.idle}</pre>
        <div className="dialogue-box">
          <p>{typedDialogue}</p>
          {isLoading && <span className="dialogue-loading">Analyzing…</span>}
          <DialogueFeedback content={gameState.dialogue} onFeedback={submitFeedback} disabled={isLoading} />
          {gameState.puzzleId && (
            <StuckButton onStuck={reportStuck} puzzleStartTime={puzzleStartTime} disabled={isLoading} />
          )}
        </div>
      </div>

      <div className="ghost-footer">
        <div className="proximity-bar">
          <div className="proximity-fill" style={{ width: `${Math.min(100, Math.max(0, gameState.proximity * 100))}%` }} />
        </div>
      </div>
    </div>
  );
};

export default Ghost;
