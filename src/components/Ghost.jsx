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

const HistoryIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="butt" strokeLinejoin="miter">
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M12 8v5l3 2" />
  </svg>
);

const KeyIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="butt" strokeLinejoin="miter">
    <rect x="4" y="9" width="6" height="6" rx="1" />
    <path d="M10 12h10" />
    <path d="M16 10v4" />
  </svg>
);

const LinkIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="butt" strokeLinejoin="miter">
    <rect x="4" y="4" width="6" height="6" rx="1" />
    <rect x="14" y="14" width="6" height="6" rx="1" />
    <path d="M10 10l4 4" />
  </svg>
);

const CloseIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18 6 6 18" />
    <path d="M6 6 18 18" />
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
    includeContext: true,
  });
  const [recentTimeline, setRecentTimeline] = useState([]);
  const [showHistory, setShowHistory] = useState(false);
  const [recentEvents, setRecentEvents] = useState([]);
  const [showEvents, setShowEvents] = useState(false);
  const [intentCandidates, setIntentCandidates] = useState([]);
  const [showIntents, setShowIntents] = useState(false);
  const [intentActions, setIntentActions] = useState([]);
  const [skills, setSkills] = useState([]);
  const [showSkills, setShowSkills] = useState(false);
  const [extensions, setExtensions] = useState([]);
  const [showExtensions, setShowExtensions] = useState(false);
  const [extensionReloading, setExtensionReloading] = useState(false);
  const [extensionTools, setExtensionTools] = useState([]);
  const [notes, setNotes] = useState([]);
  const [showNotes, setShowNotes] = useState(false);
  const [events, setEvents] = useState([]);
  const [showCalendar, setShowCalendar] = useState(false);
  const [persona, setPersona] = useState(null);
  const [personaDraft, setPersonaDraft] = useState(null);
  const [personaSaving, setPersonaSaving] = useState(false);
  const [showPersona, setShowPersona] = useState(false);
  const [notifications, setNotifications] = useState([]);
  const [showNotifications, setShowNotifications] = useState(false);
  const [systemSettings, setSystemSettings] = useState(null);

  // Derived state
  const isCompanionMode = systemStatus?.currentMode === "companion";
  const readOnlyMode = !!privacySettings?.read_only_mode;
  const hasConsent =
    !!privacySettings?.capture_consent &&
    !!privacySettings?.ai_analysis_consent &&
    !!privacySettings?.privacy_notice_acknowledged &&
    !privacySettings?.read_only_mode;
  const extensionConnectedValue = extensionConnected || systemStatus?.extensionConnected;
  const extensionHealthy = systemStatus?.extensionOperational ?? extensionConnectedValue;
  const keyConfigured = !!systemStatus?.apiKeyConfigured;
  const autoMode = !!autonomySettings?.autoPuzzleFromCompanion;
  const lastPageUpdate = systemStatus?.lastPageUpdate;
  const lastScreenshotAt = systemStatus?.lastScreenshotAt;
  const screenshotFreshWindow = useMemo(() => {
    const intervalSecs = systemSettings?.monitor_interval_secs ?? 60;
    return Math.max(120, intervalSecs * 2);
  }, [systemSettings?.monitor_interval_secs]);
  const hasRecentPage = useMemo(() => {
    if (!extensionConnectedValue || !lastPageUpdate) return false;
    return Date.now() / 1000 - lastPageUpdate < 120;
  }, [extensionConnectedValue, lastPageUpdate]);
  const hasRecentScreenshot = useMemo(() => {
    if (!lastScreenshotAt) return false;
    return Date.now() / 1000 - lastScreenshotAt < screenshotFreshWindow;
  }, [lastScreenshotAt, screenshotFreshWindow]);

  const sourceLabel = useMemo(() => {
    if (hasRecentPage) {
      return "Source: Chrome";
    }
    if ((hasRecentScreenshot || systemSettings?.monitor_enabled) && hasConsent) {
      return "Source: Screenshots";
    }
    return "Source: Waiting";
  }, [hasRecentPage, hasRecentScreenshot, systemSettings?.monitor_enabled, hasConsent]);
  const lastScanLabel = useMemo(() => {
    if (!lastScreenshotAt) return "Last scan: --";
    const delta = Math.max(0, Math.floor(Date.now() / 1000 - lastScreenshotAt));
    if (delta < 60) return `Last scan: ${delta}s ago`;
    if (delta < 3600) return `Last scan: ${Math.floor(delta / 60)}m ago`;
    return `Last scan: ${Math.floor(delta / 3600)}h ago`;
  }, [lastScreenshotAt]);

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
    actionLedger,
    showActionLedger,
    fetchActionLedger,
    closeActionLedger,
    fetchRecentEvents,
    fetchIntents,
    dismissIntent,
    createIntentAction,
    fetchIntentActions,
    autoCreateTopIntent,
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

  const loadSystemSettings = useCallback(async () => {
    const settings = await safeInvoke("get_system_settings", {}, null);
    if (settings) {
      setSystemSettings(settings);
    }
  }, []);

  useEffect(() => {
    loadPrivacy();
    loadSystemSettings();
  }, [loadPrivacy, loadSystemSettings]);

  // Listen for settings updates
  useEffect(() => {
    let unlisten = null;
    const setup = async () => {
      unlisten = await listen("settings:updated", () => {
        loadPrivacy();
        loadSystemSettings();
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
        return hasRecentPage
          ? "Auto mode: Watching your browsing..."
          : "Auto mode: Observing via screenshots...";
      }
      if (!hasRecentPage && (hasRecentScreenshot || systemSettings?.monitor_enabled) && hasConsent) {
        return "Observing via screenshots...";
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
    hasRecentPage,
    hasRecentScreenshot,
    systemSettings?.monitor_enabled,
    screenshotFreshWindow,
  ]);

  // Check if text should skip typewriter (system messages, scan results)
  const shouldSkipTypewriter = useMemo(() => {
    const systemMessages = [
      "Waiting for signal...",
      "Read-only mode. I'm just watching.",
      "Grant consent to begin.",
      "Configure your API key to start.",
      "Auto mode: Watching your browsing...",
      "Auto mode: Observing via screenshots...",
      "Companion ready. Browse to begin.",
      "Observing via screenshots...",
      "Game ready. Browse to begin.",
      "Loading puzzle...",
      "Nothing found here. Keep browsing.",
      "No mystery to investigate yet... waiting for signal.",
    ];
    return systemMessages.some(msg => displayText.startsWith(msg)) ||
           displayText.includes("Analysis failed") ||
           displayText.includes("Error:");
  }, [displayText]);

  // Typewriter effect - only triggers on actual text change
  useEffect(() => {
    if (typewriterRef.current) {
      clearInterval(typewriterRef.current);
    }
    if (displayText === lastTextRef.current) return;
    lastTextRef.current = displayText;

    // Skip typewriter for system messages and errors
    if (shouldSkipTypewriter) {
      setTypedDialogue(displayText);
      return;
    }

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
  }, [displayText, shouldSkipTypewriter]);

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

  useEffect(() => {
    let mounted = true;
    const loadTimeline = async () => {
      try {
        const entries = await invoke("get_timeline", { limit: 6, offset: 0 });
        if (mounted && Array.isArray(entries)) {
          setRecentTimeline(entries);
        }
      } catch (err) {
        console.error("Failed to load timeline", err);
      }
    };
    loadTimeline();
    const timer = setInterval(loadTimeline, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadEvents = async () => {
      const entries = await fetchRecentEvents?.();
      if (mounted && Array.isArray(entries)) {
        setRecentEvents(entries);
      }
    };
    loadEvents();
    const timer = setInterval(loadEvents, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, [fetchRecentEvents]);

  useEffect(() => {
    let mounted = true;
    const loadIntents = async () => {
      const intents = await fetchIntents?.();
      if (mounted && Array.isArray(intents)) {
        setIntentCandidates(intents);
      }
    };
    loadIntents();
    const timer = setInterval(loadIntents, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, [fetchIntents]);

  useEffect(() => {
    let mounted = true;
    const loadIntentActions = async () => {
      const actions = await fetchIntentActions?.();
      if (mounted && Array.isArray(actions)) {
        setIntentActions(actions);
      }
    };
    loadIntentActions();
    const timer = setInterval(loadIntentActions, 10000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, [fetchIntentActions]);

  useEffect(() => {
    let mounted = true;
    const loadSkills = async () => {
      const entries = await safeInvoke("list_skills", {}, []);
      if (mounted && Array.isArray(entries)) {
        setSkills(entries);
      }
    };
    loadSkills();
    const timer = setInterval(loadSkills, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadExtensions = async () => {
      const entries = await safeInvoke("list_extensions", {}, []);
      if (mounted && Array.isArray(entries)) {
        setExtensions(entries);
      }
      const tools = await safeInvoke("list_extension_tools", {}, []);
      if (mounted && Array.isArray(tools)) {
        setExtensionTools(tools);
      }
    };
    loadExtensions();
    const timer = setInterval(loadExtensions, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadNotes = async () => {
      const entries = await safeInvoke("list_notes", {}, []);
      if (mounted && Array.isArray(entries)) {
        setNotes(entries);
      }
    };
    loadNotes();
    const timer = setInterval(loadNotes, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadCalendar = async () => {
      const entries = await safeInvoke("get_upcoming_events", { limit: 5 }, []);
      if (mounted && Array.isArray(entries)) {
        setEvents(entries);
      }
    };
    loadCalendar();
    const timer = setInterval(loadCalendar, 60000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadPersona = async () => {
      const profile = await safeInvoke("get_persona", {}, null);
      if (mounted && profile) {
        setPersona(profile);
        setPersonaDraft(profile);
      }
    };
    loadPersona();
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadNotifications = async () => {
      const entries = await safeInvoke("list_notifications", { limit: 10 }, []);
      if (mounted && Array.isArray(entries)) {
        setNotifications(entries);
      }
    };
    loadNotifications();
    const timer = setInterval(loadNotifications, 30000);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (!hasConsent || readOnlyMode || !isCompanionMode) return;
    const timer = setInterval(() => {
      autoCreateTopIntent?.();
    }, 60000);
    return () => clearInterval(timer);
  }, [autoCreateTopIntent, hasConsent, readOnlyMode, isCompanionMode]);


  const aggregatedTimeline = useMemo(() => {
    const output = [];
    for (const entry of recentTimeline) {
      const last = output[output.length - 1];
      if (
        last &&
        last.summary === entry.summary &&
        last.reason === entry.reason &&
        last.status === entry.status
      ) {
        last.count += 1;
        continue;
      }
      output.push({ ...entry, count: 1 });
    }
    return output;
  }, [recentTimeline]);

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
      previewPolicy: privacySettings.preview_policy || "always",
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
        const response = await invoke("quick_ask", {
          prompt: quickAsk.prompt.trim(),
          includeContext: quickAsk.includeContext,
        });
        setQuickAsk({
          prompt: "",
          response,
          error: "",
          isLoading: false,
          isOpen: false,
          includeContext: quickAsk.includeContext,
        });
      } catch (err) {
        const message = typeof err === "string" ? err : err?.message || "Quick ask failed";
        setQuickAsk((prev) => ({ ...prev, isLoading: false, error: message }));
      }
    },
    [quickAsk.prompt, quickAsk.includeContext]
  );

  const handleIntentApply = useCallback(
    async (intent) => {
      if (!intent?.summary) return;
      await createIntentAction?.(intent.summary);
    },
    [createIntentAction]
  );

  // Compute system status for display
  const systemState = useMemo(() => {
    if (!keyConfigured) return { status: "error", label: "No API Key" };
    if (!hasConsent) return { status: "warning", label: "Consent needed" };
    if (readOnlyMode) return { status: "info", label: "Read-only" };
    if (!extensionConnectedValue) return { status: "warning", label: "No extension" };
    if (!extensionHealthy) return { status: "warning", label: "Extension stale" };
    return { status: "ok", label: isCompanionMode ? "Companion" : "Game" };
  }, [keyConfigured, hasConsent, readOnlyMode, extensionConnectedValue, extensionHealthy, isCompanionMode]);

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
        {extensionConnectedValue && !systemStatus?.extensionProtocolVersion && !systemStatus?.lastExtensionHello && (
          <button
            type="button"
            className="ghost-alert warning"
            onClick={() => openSettings("extensions")}
          >
            <StatusDot status="warning" pulse={false} />
            <span>Extension handshake missing. Reload extension.</span>
          </button>
        )}
        {extensionConnectedValue && systemStatus?.extensionProtocolVersion === "legacy" && (
          <button
            type="button"
            className="ghost-alert info"
            onClick={() => openSettings("extensions")}
          >
            <StatusDot status="info" pulse={false} />
            <span>Legacy extension detected. Update for handshake support.</span>
          </button>
        )}
        <div className="ghost-source">
          <span>{sourceLabel}</span>
          <span>{lastScanLabel}</span>
        </div>
      </header>

      {/* Main content: Sprite + Bubble */}
      <main className="ghost-body">
        <div className="ghost-sprite-column">
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
          <div className="ghost-side-actions">
            <button
              type="button"
              className="ghost-icon-btn"
              onClick={() => openSettings("integrations")}
              aria-label="Open integrations"
              title="Integrations"
            >
              <LinkIcon />
            </button>
            <button
              type="button"
              className="ghost-icon-btn"
              onClick={() => openSettings("keys")}
              aria-label="Open keys and models"
              title="Keys & Models"
            >
              <KeyIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showHistory ? "active" : ""}`}
              onClick={() => setShowHistory((prev) => !prev)}
              aria-label="Toggle recent activity"
              title="Recent activity"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showActionLedger ? "active" : ""}`}
              onClick={() => (showActionLedger ? closeActionLedger() : fetchActionLedger())}
              aria-label="Toggle action ledger"
              title="Action ledger"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showEvents ? "active" : ""}`}
              onClick={() => setShowEvents((prev) => !prev)}
              aria-label="Toggle recent events"
              title="Recent events"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showIntents ? "active" : ""}`}
              onClick={() => setShowIntents((prev) => !prev)}
              aria-label="Toggle intent ideas"
              title="Intent ideas"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showSkills ? "active" : ""}`}
              onClick={() => setShowSkills((prev) => !prev)}
              aria-label="Toggle skills"
              title="Skills"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showExtensions ? "active" : ""}`}
              onClick={() => setShowExtensions((prev) => !prev)}
              aria-label="Toggle extensions"
              title="Extensions"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showNotes ? "active" : ""}`}
              onClick={() => setShowNotes((prev) => !prev)}
              aria-label="Toggle notes"
              title="Notes"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showCalendar ? "active" : ""}`}
              onClick={() => setShowCalendar((prev) => !prev)}
              aria-label="Toggle calendar"
              title="Calendar"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showPersona ? "active" : ""}`}
              onClick={() => setShowPersona((prev) => !prev)}
              aria-label="Toggle persona"
              title="Persona"
            >
              <HistoryIcon />
            </button>
            <button
              type="button"
              className={`ghost-icon-btn ${showNotifications ? "active" : ""}`}
              onClick={() => setShowNotifications((prev) => !prev)}
              aria-label="Toggle notifications"
              title="Notifications"
            >
              <HistoryIcon />
            </button>
          </div>
        </div>

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
          {companionBehavior?.trigger_context && (
            <div className="companion-line subtle">Why: {companionBehavior.trigger_context}</div>
          )}
          {intentCandidates.length > 0 && !companionBehavior?.suggestion && (
            <div className="companion-line">
              Next idea: {intentCandidates[0].summary}
            </div>
          )}
          {intentCandidates.length > 0 && !companionBehavior?.trigger_context && (
            <div className="companion-line subtle">
              Why: {intentCandidates[0].reason}
            </div>
          )}
          {intentCandidates.length > 0 && !companionBehavior?.suggestion && (
            <div className="intent-actions">
              <button
                type="button"
                className="mini-btn"
                onClick={() => handleIntentApply(intentCandidates[0])}
              >
                Create action
              </button>
              <button
                type="button"
                className="mini-btn subtle"
                onClick={() => {
                  const summary = intentCandidates[0].summary;
                  dismissIntent?.(summary);
                  setIntentCandidates((prev) => prev.filter((item) => item.summary !== summary));
                }}
              >
                Dismiss
              </button>
            </div>
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
              onClick={handleAnalyze}
              disabled={!hasConsent || isLoading}
              title="Manual screenshot scan"
            >
              Scan
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
              <label className="quick-ask-toggle">
                <input
                  type="checkbox"
                  checked={quickAsk.includeContext}
                  onChange={(e) =>
                    setQuickAsk((prev) => ({ ...prev, includeContext: e.target.checked }))
                  }
                />
                <span>Include context</span>
              </label>
            </form>
          )}

          {/* Clear quick ask response */}
          {quickAsk.response && (
            <button
              type="button"
              className="mini-btn subtle"
              onClick={() => setQuickAsk({ prompt: "", response: "", error: "", isLoading: false, isOpen: false, includeContext: true })}
            >
              Clear response
            </button>
          )}

          {showHistory && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowHistory(false)}
                aria-label="Close recent activity"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Recent activity"
                onClick={(event) => event.stopPropagation()}
              >
              <div className="timeline-header">
                <span>Recent activity</span>
                <div className="timeline-actions">
                  <button
                    type="button"
                    className="ghost-icon-btn compact"
                    onClick={() => setShowHistory(false)}
                    aria-label="Close recent activity"
                    title="Close"
                  >
                    <CloseIcon />
                  </button>
                </div>
              </div>
              {aggregatedTimeline.length === 0 ? (
                <div className="timeline-empty">No recent activity.</div>
              ) : (
                <div className="timeline-list">
                  {aggregatedTimeline.map((entry) => (
                    <div key={entry.id} className="timeline-item">
                      <div className="timeline-summary">
                        {entry.summary}
                        {entry.count > 1 && (
                          <span className="timeline-count">×{entry.count}</span>
                        )}
                      </div>
                      {entry.reason && <div className="timeline-reason">{entry.reason}</div>}
                      <div className="timeline-meta">
                        <span>{new Date(entry.timestamp * 1000).toLocaleTimeString()}</span>
                        <span className="timeline-status">{entry.status}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              </div>
            </>
          )}

          {showActionLedger && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={closeActionLedger}
                aria-label="Close action ledger"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Action ledger"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Action ledger</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={async () => {
                        if (extensionReloading) return;
                        setExtensionReloading(true);
                        const entries = await safeInvoke("reload_extensions", {}, []);
                        if (Array.isArray(entries)) {
                          setExtensions(entries);
                        }
                        setExtensionReloading(false);
                      }}
                      aria-label="Reload extensions"
                      title="Reload"
                    >
                      {extensionReloading ? "..." : "Reload"}
                    </button>
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={closeActionLedger}
                      aria-label="Close action ledger"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {actionLedger.length === 0 ? (
                  <div className="timeline-empty">No action entries.</div>
                ) : (
                  <div className="timeline-list">
                    {actionLedger.map((entry) => (
                      <div key={`${entry.action_id}-${entry.timestamp}`} className="timeline-item">
                        <div className="timeline-summary">
                          {entry.description || entry.action_type}
                        </div>
                        {entry.reason && <div className="timeline-reason">Why: {entry.reason}</div>}
                        <div className="timeline-meta">
                          <span>{new Date(entry.timestamp * 1000).toLocaleTimeString()}</span>
                          <span className="timeline-status">{entry.status}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showEvents && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowEvents(false)}
                aria-label="Close recent events"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Recent events"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Recent events</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowEvents(false)}
                      aria-label="Close recent events"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {recentEvents.length === 0 ? (
                  <div className="timeline-empty">No recent events.</div>
                ) : (
                  <div className="timeline-list">
                    {recentEvents.map((entry) => (
                      <div key={entry.id} className="timeline-item">
                        <div className="timeline-summary">{entry.summary}</div>
                        {entry.detail && <div className="timeline-reason">{entry.detail}</div>}
                        <div className="timeline-meta">
                          <span>{new Date(entry.timestamp * 1000).toLocaleTimeString()}</span>
                          <span className="timeline-status">{entry.kind}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showIntents && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowIntents(false)}
                aria-label="Close intent ideas"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Intent ideas"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Intent ideas</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowIntents(false)}
                      aria-label="Close intent ideas"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {intentActions.length > 0 && (
                  <div className="timeline-subheader">Pending intent actions</div>
                )}
                {intentActions.length > 0 && (
                  <div className="timeline-list">
                    {intentActions.map((action) => (
                      <div key={action.id} className="timeline-item">
                        <div className="timeline-summary">{action.description}</div>
                        {action.reason && <div className="timeline-reason">Why: {action.reason}</div>}
                        <div className="timeline-meta">
                          <span>{action.action_type}</span>
                          <span className="timeline-status">{action.risk_level}</span>
                          {action.target && <span className="timeline-status">{action.target}</span>}
                        </div>
                        <div className="intent-actions">
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => approveAction(action.id)}
                          >
                            Approve
                          </button>
                          <button
                            type="button"
                            className="mini-btn subtle"
                            onClick={() => denyAction(action.id)}
                          >
                            Deny
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
                {intentCandidates.length === 0 ? (
                  <div className="timeline-empty">No intent ideas yet.</div>
                ) : (
                  <div className="timeline-list">
                    {intentCandidates.map((intent) => (
                      <div key={intent.id} className="timeline-item">
                        <div className="timeline-summary">{intent.summary}</div>
                        <div className="timeline-reason">Why: {intent.reason}</div>
                        <div className="timeline-meta">
                          <span>Confidence: {(intent.confidence * 100).toFixed(0)}%</span>
                          <span className="timeline-status">{intent.kind}</span>
                        </div>
                        <div className="intent-actions">
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => handleIntentApply(intent)}
                          >
                            Create action
                          </button>
                          <button
                            type="button"
                            className="mini-btn subtle"
                            onClick={() => {
                              dismissIntent?.(intent.summary);
                              setIntentCandidates((prev) =>
                                prev.filter((item) => item.summary !== intent.summary)
                              );
                            }}
                          >
                            Dismiss
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showSkills && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowSkills(false)}
                aria-label="Close skills"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Skills"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Skills</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowSkills(false)}
                      aria-label="Close skills"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {skills.length === 0 ? (
                  <div className="timeline-empty">No skills yet.</div>
                ) : (
                  <div className="timeline-list">
                    {skills.map((skill) => (
                      <div key={skill.id} className="timeline-item">
                        <div className="timeline-summary">{skill.title}</div>
                        <div className="timeline-reason">Trigger: {skill.trigger}</div>
                        <div className="timeline-meta">
                          <span>Uses: {skill.usage_count}</span>
                          <span className="timeline-status">{skill.action_type}</span>
                        </div>
                        {skill.description && (
                          <div className="timeline-reason">{skill.description}</div>
                        )}
                        <div className="intent-actions">
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => safeInvoke("execute_skill", { skillId: skill.id }, null)}
                          >
                            Run
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showExtensions && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowExtensions(false)}
                aria-label="Close extensions"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Extensions"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Extensions</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowExtensions(false)}
                      aria-label="Close extensions"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {extensions.length === 0 ? (
                  <div className="timeline-empty">No extensions found.</div>
                ) : (
                  <div className="timeline-list">
                    {extensions.map((ext) => (
                      <div key={ext.id} className="timeline-item">
                        <div className="timeline-summary">{ext.name}</div>
                        {ext.last_error && (
                          <div className="timeline-reason">Error: {ext.last_error}</div>
                        )}
                        <div className="timeline-meta">
                          <span>{ext.version}</span>
                          <span className="timeline-status">{ext.loaded ? "loaded" : "error"}</span>
                        </div>
                        {extensionTools.find((item) => item.extension_id === ext.id)?.tools?.length > 0 && (
                          <div className="timeline-reason">
                            {extensionTools
                              .find((item) => item.extension_id === ext.id)
                              .tools.map((tool) => (
                                <div key={`${ext.id}-${tool.name}`} className="tool-row">
                                  <span className="tool-name">{tool.name}</span>
                                  {tool.risk_level && (
                                    <span className={`pending-tag ${tool.risk_level}`}>
                                      {tool.risk_level}
                                    </span>
                                  )}
                                  {tool.requires_approval && (
                                    <span className="pending-tag sandbox">Approval</span>
                                  )}
                                  {tool.description && (
                                    <span className="tool-desc">{tool.description}</span>
                                  )}
                                  {tool.approval_reason && (
                                    <span className="tool-desc">{tool.approval_reason}</span>
                                  )}
                                  {tool.args_schema && (
                                    <span className="tool-schema">
                                      Schema: {JSON.stringify(tool.args_schema, null, 0)}
                                    </span>
                                  )}
                                </div>
                              ))}
                          </div>
                        )}
                        <div className="intent-actions">
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => safeInvoke("execute_extension", { id: ext.id, args: [] }, null)}
                            disabled={!ext.loaded}
                          >
                            Run
                          </button>
                          {extensionTools
                            .find((item) => item.extension_id === ext.id)
                            ?.tools?.map((tool) => (
                              <button
                                key={`${ext.id}-${tool.name}`}
                                type="button"
                                className="mini-btn subtle"
                                onClick={() =>
                                  safeInvoke(
                                    "request_extension_tool_action",
                                    { extension_id: ext.id, tool_name: tool.name, args: [] },
                                    null
                                  )
                                }
                                disabled={!ext.loaded}
                              >
                                {tool.name}
                              </button>
                            ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showNotes && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowNotes(false)}
                aria-label="Close notes"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Notes"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Notes</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowNotes(false)}
                      aria-label="Close notes"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {notes.length === 0 ? (
                  <div className="timeline-empty">No notes saved yet.</div>
                ) : (
                  <div className="timeline-list">
                    {notes.slice(0, 6).map((note) => (
                      <div key={note.id} className="timeline-item">
                        <div className="timeline-summary">{note.title}</div>
                        <div className="timeline-reason">{note.body}</div>
                        <div className="timeline-meta">
                          <span>{new Date(note.updated_at * 1000).toLocaleDateString()}</span>
                          <span className="timeline-status">{note.pinned ? "pinned" : "note"}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showCalendar && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowCalendar(false)}
                aria-label="Close calendar"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Calendar"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Upcoming</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowCalendar(false)}
                      aria-label="Close calendar"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {events.length === 0 ? (
                  <div className="timeline-empty">No upcoming events.</div>
                ) : (
                  <div className="timeline-list">
                    {events.map((event) => (
                      <div key={event.id} className="timeline-item">
                        <div className="timeline-summary">{event.summary}</div>
                        <div className="timeline-meta">
                          <span>{new Date(event.starts_at * 1000).toLocaleString()}</span>
                          {event.location && <span className="timeline-status">{event.location}</span>}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

          {showPersona && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowPersona(false)}
                aria-label="Close persona"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Persona"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Persona</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowPersona(false)}
                      aria-label="Close persona"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {!personaDraft ? (
                  <div className="timeline-empty">Persona unavailable.</div>
                ) : (
                  <div className="timeline-list">
                    <div className="timeline-item">
                      <div className="timeline-summary">Profile</div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Name</span>
                        </div>
                        <div className="param-input">
                          <input
                            className="text-input"
                            value={personaDraft.name}
                            onChange={(event) =>
                              setPersonaDraft((prev) => ({ ...prev, name: event.target.value }))
                            }
                          />
                        </div>
                      </div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Description</span>
                        </div>
                        <div className="param-input">
                          <input
                            className="text-input"
                            value={personaDraft.description}
                            onChange={(event) =>
                              setPersonaDraft((prev) => ({ ...prev, description: event.target.value }))
                            }
                          />
                        </div>
                      </div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Tone</span>
                        </div>
                        <div className="param-input">
                          <input
                            className="text-input"
                            value={personaDraft.tone}
                            onChange={(event) =>
                              setPersonaDraft((prev) => ({ ...prev, tone: event.target.value }))
                            }
                          />
                        </div>
                      </div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Hint density</span>
                        </div>
                        <div className="param-input">
                          <input
                            className="text-input"
                            type="number"
                            min="0"
                            max="1"
                            step="0.1"
                            value={personaDraft.hint_density}
                            onChange={(event) =>
                              setPersonaDraft((prev) => ({
                                ...prev,
                                hint_density: Number(event.target.value),
                              }))
                            }
                          />
                        </div>
                      </div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Action aggressiveness</span>
                        </div>
                        <div className="param-input">
                          <input
                            className="text-input"
                            type="number"
                            min="0"
                            max="1"
                            step="0.1"
                            value={personaDraft.action_aggressiveness}
                            onChange={(event) =>
                              setPersonaDraft((prev) => ({
                                ...prev,
                                action_aggressiveness: Number(event.target.value),
                              }))
                            }
                          />
                        </div>
                      </div>
                      <div className="param-row">
                        <div className="param-meta">
                          <span className="param-label">Auto intents</span>
                        </div>
                        <div className="param-input">
                          <label className="checkbox-row compact">
                            <input
                              type="checkbox"
                              checked={!!personaDraft.allow_auto_intents}
                              onChange={(event) =>
                                setPersonaDraft((prev) => ({
                                  ...prev,
                                  allow_auto_intents: event.target.checked,
                                }))
                              }
                            />
                            <span>{personaDraft.allow_auto_intents ? "Enabled" : "Disabled"}</span>
                          </label>
                        </div>
                      </div>
                      <div className="button-row">
                        <button
                          type="button"
                          className="ghost-button"
                          onClick={async () => {
                            if (personaSaving) return;
                            setPersonaSaving(true);
                            const updated = await safeInvoke("set_persona", { profile: personaDraft }, null);
                            if (updated) {
                              setPersona(updated);
                              setPersonaDraft(updated);
                            }
                            setPersonaSaving(false);
                          }}
                        >
                          {personaSaving ? "Saving…" : "Save persona"}
                        </button>
                        <button
                          type="button"
                          className="ghost-button"
                          onClick={() => setPersonaDraft(persona)}
                        >
                          Reset
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </>
          )}

          {showNotifications && (
            <>
              <button
                type="button"
                className="ghost-overlay"
                onClick={() => setShowNotifications(false)}
                aria-label="Close notifications"
              />
              <div
                className="timeline-popover"
                role="dialog"
                aria-label="Notifications"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="timeline-header">
                  <span>Notifications</span>
                  <div className="timeline-actions">
                    <button
                      type="button"
                      className="ghost-icon-btn compact"
                      onClick={() => setShowNotifications(false)}
                      aria-label="Close notifications"
                      title="Close"
                    >
                      <CloseIcon />
                    </button>
                  </div>
                </div>
                {notifications.length === 0 ? (
                  <div className="timeline-empty">No notifications.</div>
                ) : (
                  <div className="timeline-list">
                    {notifications.map((note) => (
                      <div key={note.id} className="timeline-item">
                        <div className="timeline-summary">{note.title}</div>
                        <div className="timeline-reason">{note.body}</div>
                        <div className="timeline-meta">
                          <span>{new Date(note.timestamp * 1000).toLocaleTimeString()}</span>
                          <span className="timeline-status">{note.level}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
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
          <div className="pending-desc">
            {pendingActions[0].description}
            {pendingActions[0].action_type?.startsWith("intent.") && (
              <span className="pending-tag intent">Intent</span>
            )}
            {pendingActions[0].action_type?.startsWith("sandbox.") && (
              <span className="pending-tag sandbox">Sandbox</span>
            )}
            {pendingActions[0].action_type?.startsWith("browser.") && (
              <span className="pending-tag browser">Browser</span>
            )}
          </div>
          {pendingActions[0].reason && (
            <div className="pending-why">Why: {pendingActions[0].reason}</div>
          )}
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
          <div className="pending-desc">
            {actionPreview.action?.description}
            {actionPreview.action?.action_type?.startsWith("intent.") && (
              <span className="pending-tag intent">Intent</span>
            )}
            {actionPreview.action?.action_type?.startsWith("sandbox.") && (
              <span className="pending-tag sandbox">Sandbox</span>
            )}
            {actionPreview.action?.action_type?.startsWith("browser.") && (
              <span className="pending-tag browser">Browser</span>
            )}
          </div>
          {actionPreview.action?.target && (
            <div className="pending-detail">Target: {actionPreview.action.target}</div>
          )}
          {actionPreview.action?.reason && (
            <div className="pending-why">Why: {actionPreview.action.reason}</div>
          )}
          {actionPreview.action?.risk_level && (
            <div className="pending-risk">Risk: {actionPreview.action.risk_level}</div>
          )}

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
                <div
                  key={name}
                  className={`param-row ${param.requires_approval ? "param-sensitive" : ""}`}
                >
                  <div className="param-meta">
                    <span className="param-label">{param.label}</span>
                    {param.description && (
                      <span className="param-help">{param.description}</span>
                    )}
                    {param.help_text && (
                      <span className="param-help">{param.help_text}</span>
                    )}
                    {param.constraints &&
                      (param.constraints.min !== null ||
                        param.constraints.max !== null ||
                        param.constraints.max_length !== null ||
                        param.unit) && (
                        <span className="param-hint">
                          {param.constraints.min !== null && param.constraints.min !== undefined
                            ? `Min ${param.constraints.min}`
                            : ""}
                          {param.constraints.max !== null && param.constraints.max !== undefined
                            ? `${param.constraints.min !== null && param.constraints.min !== undefined ? " · " : ""}Max ${
                                param.constraints.max
                              }`
                            : ""}
                          {param.constraints.max_length !== null &&
                          param.constraints.max_length !== undefined
                            ? `${
                                (param.constraints.min !== null && param.constraints.min !== undefined) ||
                                (param.constraints.max !== null && param.constraints.max !== undefined)
                                  ? " · "
                                  : ""
                              }Max length ${param.constraints.max_length}`
                            : ""}
                          {param.unit ? ` · Unit ${param.unit}` : ""}
                        </span>
                      )}
                    {param.requires_approval && (
                      <span className="param-alert">
                        Approval required{param.approval_reason ? `: ${param.approval_reason}` : ""}
                      </span>
                    )}
                  </div>
                  <div className="param-input">
                    {param.param_type === "select" && Array.isArray(param.constraints?.options) ? (
                      <select
                        className="select-control"
                        value={param.value}
                        onChange={(event) => editPreviewParam(name, event.target.value)}
                      >
                        {!param.constraints.required && <option value="">(none)</option>}
                        {param.constraints.options.map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    ) : param.param_type === "boolean" ? (
                      <label className="checkbox-row compact">
                        <input
                          type="checkbox"
                          checked={Boolean(param.value)}
                          onChange={(event) => editPreviewParam(name, event.target.checked)}
                        />
                        <span>{Boolean(param.value) ? "Enabled" : "Disabled"}</span>
                      </label>
                    ) : (
                      <input
                        className="text-input"
                        type={param.param_type === "number" || param.param_type === "duration" ? "number" : "text"}
                        value={param.value ?? ""}
                        placeholder={param.label}
                        min={
                          param.param_type === "number" || param.param_type === "duration"
                            ? param.constraints?.min ?? undefined
                            : undefined
                        }
                        max={
                          param.param_type === "number" || param.param_type === "duration"
                            ? param.constraints?.max ?? undefined
                            : undefined
                        }
                        maxLength={
                          param.param_type !== "number" && param.param_type !== "duration"
                            ? param.constraints?.max_length ?? undefined
                            : undefined
                        }
                        onChange={(event) => {
                          const raw = event.target.value;
                          const normalized =
                            param.param_type === "number" || param.param_type === "duration"
                              ? Number(raw)
                              : raw;
                          editPreviewParam(name, normalized);
                        }}
                      />
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {actionPreview.requires_approval && (
            <div className="pending-why">
              Approval required: {actionPreview.approval_summary || "Sensitive parameters"}
            </div>
          )}

          <div className="pending-actions">
            <button type="button" className="mini-btn approve" onClick={approvePreview}>
              {actionPreview.requires_approval ? "Approve" : "Execute"}
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
