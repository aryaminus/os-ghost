import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "../utils/data";
import SettingsSidebar from "./SettingsSidebar";
import GeneralSection from "./sections/GeneralSection";
import PrivacySection from "./sections/PrivacySection";
import ExtensionsSection from "./sections/ExtensionsSection";
import KeysSection from "./sections/KeysSection";
import AutonomySection from "./sections/AutonomySection";
import SandboxSection from "./sections/SandboxSection";
import DeveloperSection from "./sections/DeveloperSection";
import IntegrationsSection from "./sections/IntegrationsSection";
import VisualAutomationSection from "./sections/VisualAutomationSection";
import WorkflowsSection from "./sections/WorkflowsSection";

const DEFAULT_SETTINGS_STATE = {
  privacy: null,
  privacyNotice: "",
  systemStatus: null,
  autonomySettings: null,
  intelligentMode: null,
  sandboxSettings: null,
  systemSettings: null,
  captureSettings: null,
  schedulerSettings: null,
  pairingStatus: null,
  permissionDiagnostics: null,
  recentTimeline: [],
  calendarSettings: null,
  notes: [],
  filesSettings: null,
  emailSettings: null,
};

const SECTIONS = Object.freeze([
  { id: "general", label: "General" },
  { id: "privacy", label: "Privacy & Security" },
  { id: "visual", label: "Visual Automation" },
  { id: "workflows", label: "Workflows" },
  { id: "extensions", label: "Extensions" },
  { id: "keys", label: "Keys & Models" },
  { id: "autonomy", label: "Autonomy" },
  { id: "sandbox", label: "Sandbox" },
  { id: "integrations", label: "Integrations" },
  { id: "developer", label: "Developer" },
]);

const DEV_MODE_KEY = "osghost_dev_mode";

const SettingsApp = () => {
  const [activeSection, setActiveSection] = useState("general");
  const [settingsState, setSettingsState] = useState(DEFAULT_SETTINGS_STATE);
  const [loadingState, setLoadingState] = useState(true);
  const hasLoadedRef = useRef(false);
  const [devModeEnabled, setDevModeEnabled] = useState(() => {
    try {
      return window.localStorage.getItem(DEV_MODE_KEY) === "true";
    } catch {
      return false;
    }
  });

  const refreshSettings = useCallback(async ({ silent = false } = {}) => {
    if (!hasLoadedRef.current && !silent) {
      setLoadingState(true);
    }
    const state = await safeInvoke("get_settings_state", {}, null);
    if (state) {
      const normalizedStatus = state.system_status
        ? {
            chromeInstalled: state.system_status.chrome_installed,
            chromePath: state.system_status.chrome_path,
            extensionConnected: state.system_status.extension_connected,
            extensionOperational: state.system_status.extension_operational,
            lastExtensionHeartbeat: state.system_status.last_extension_heartbeat,
            lastExtensionHello: state.system_status.last_extension_hello,
            extensionProtocolVersion: state.system_status.extension_protocol_version,
            extensionVersion: state.system_status.extension_version,
            extensionId: state.system_status.extension_id,
            extensionCapabilities: state.system_status.extension_capabilities,
            mcpBrowserConnected: state.system_status.mcp_browser_connected,
            lastPageUpdate: state.system_status.last_page_update,
            apiKeyConfigured: state.system_status.api_key_configured,
            apiKeySource: state.system_status.api_key_source,
            lastKnownUrl: state.system_status.last_known_url,
            lastScreenshotAt: state.system_status.last_screenshot_at,
            lastTabScreenshotAt: state.system_status.last_tab_screenshot_at,
            activeProvider: state.system_status.active_provider,
            currentMode: state.system_status.current_mode,
            preferredMode: state.system_status.preferred_mode,
            autoPuzzleFromCompanion: state.system_status.auto_puzzle_from_companion,
            intentCooldownSecs: state.autonomy_settings?.intent_cooldown_secs ?? 0,
          }
        : null;

      setSettingsState({
        privacy: state.privacy,
        privacyNotice: state.privacy_notice || "",
        systemStatus: normalizedStatus,
        autonomySettings: state.autonomy_settings,
        intelligentMode: state.intelligent_mode,
        sandboxSettings: state.sandbox_settings,
        systemSettings: state.system_settings,
        captureSettings: state.capture_settings,
        schedulerSettings: state.scheduler_settings,
        pairingStatus: state.pairing_status,
        permissionDiagnostics: state.permission_diagnostics,
        recentTimeline: state.recent_timeline || [],
        calendarSettings: state.calendar_settings,
        notes: state.notes || [],
        filesSettings: state.files_settings || null,
        emailSettings: state.email_settings || null,
      });
    }
    if (!hasLoadedRef.current) {
      hasLoadedRef.current = true;
      setLoadingState(false);
    }
  }, []);

  const handleSettingsUpdated = useCallback(async () => {
    await refreshSettings({ silent: true });
    await emit("settings:updated", { timestamp: Date.now() });
  }, [refreshSettings]);

  useEffect(() => {
    refreshSettings();
  }, [refreshSettings]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const initial = params.get("section");
    if (initial) {
      setActiveSection(initial);
    }
  }, []);

  useEffect(() => {
    let unlisten = null;
    const setup = async () => {
      unlisten = await listen("settings:navigate", (event) => {
        const next = event?.payload?.section || event?.payload || "general";
        if (typeof next === "string") {
          setActiveSection(next);
        }
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleSectionChange = useCallback((id) => {
    setActiveSection(id);
  }, []);

  const openSection = useCallback(async (id) => {
    setActiveSection(id);
    await invoke("open_settings", { section: id });
  }, []);

  const toggleDevMode = useCallback(() => {
    setDevModeEnabled((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(DEV_MODE_KEY, String(next));
      } catch {
        // no-op
      }
      return next;
    });
  }, []);

  const sectionList = useMemo(() => {
    if (devModeEnabled) return SECTIONS;
    return SECTIONS.filter((section) => section.id !== "developer");
  }, [devModeEnabled]);

  return (
    <div className="settings-shell">
      <header className="settings-header">
        <div className="settings-title">
          <span className="settings-title-text">System Settings</span>
          <span className="settings-subtitle">OS Ghost control center</span>
        </div>
        <div className="settings-header-actions">
          <button
            type="button"
            className="settings-ghost-button"
            onClick={() => openSection("privacy")}
          >
            Review consent
          </button>
        </div>
      </header>

      <div className="settings-body">
        <SettingsSidebar
          sections={sectionList}
          activeSection={activeSection}
          onSelect={handleSectionChange}
        />
        <main className="settings-content" aria-live="polite">
          {loadingState ? (
            <div className="settings-loading">Loading settings…</div>
          ) : (
            <>
              {activeSection === "general" && (
                <GeneralSection
                  settingsState={settingsState}
                  devModeEnabled={devModeEnabled}
                  onToggleDevMode={toggleDevMode}
                  onOpenSection={openSection}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "privacy" && (
                <PrivacySection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "visual" && (
                <VisualAutomationSection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "workflows" && (
                <WorkflowsSection
                  settingsState={settingsState}
                />
              )}
              {activeSection === "extensions" && (
                <ExtensionsSection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "keys" && (
                <KeysSection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "autonomy" && (
                <AutonomySection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "sandbox" && (
                <SandboxSection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "integrations" && (
                <IntegrationsSection
                  settingsState={settingsState}
                  onSettingsUpdated={handleSettingsUpdated}
                />
              )}
              {activeSection === "developer" && devModeEnabled && (
                <DeveloperSection
                  settingsState={settingsState}
                  onSettingsUpdated={refreshSettings}
                />
              )}
            </>
          )}
        </main>
      </div>
    </div>
  );
};

export default SettingsApp;
