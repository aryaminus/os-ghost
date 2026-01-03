# Permission Justification for OS Ghost Bridge

This document explains why each permission is required for the OS Ghost Bridge Chrome extension to function properly. Use this information when submitting to the Chrome Web Store.

## Required Permissions

### `nativeMessaging`

**Justification**: This permission is the **core functionality** of the extension. OS Ghost Bridge exists solely to connect Chrome browser with The OS Ghost desktop application (built with Tauri). Without native messaging, the extension cannot communicate with the desktop app and becomes non-functional.

**How it's used**:

- Send page navigation events to the desktop app
- Receive commands to inject visual effects
- Establish connection status monitoring

### `activeTab`

**Justification**: Required to access the URL and title of the currently active tab when the user is playing the game.

**How it's used**:

- Read current page URL to check against puzzle solutions
- Read page title for game context
- Access is only used when extension is active

### `tabs`

**Justification**: Required to monitor tab navigation and switching events for gameplay.

**How it's used**:

- `chrome.tabs.onUpdated` - Detect when a page finishes loading
- `chrome.tabs.onActivated` - Detect when user switches tabs
- `chrome.tabs.query` - Get active tab information
- These events drive the core gameplay loop

### `scripting`

**Justification**: Required to inject visual effects into web pages as game feedback.

**How it's used**:

- Content script runs on pages to enable effects
- Visual effects include: glitch, scanlines, static noise, pulse glow, flicker
- All effects are temporary and cosmetic (do not modify page content)

### `storage`

**Justification**: Required to persist connection status between popup opens.

**How it's used**:

- Store boolean `appConnected` status
- Popup reads this to show connection indicator
- No sensitive data is stored

## Host Permissions

### `<all_urls>`

**Justification**: The OS Ghost is a meta-game where puzzles can lead players to **any website on the internet**. The game cannot predict which websites will be relevant to each dynamically-generated puzzle.

**Why we can't narrow this permission**:

- Puzzles are AI-generated based on current events and user context
- Limiting to specific URLs would break the core gameplay experience
- The game's value proposition is that "the entire internet is your puzzle box"

**User safety measures**:

- Extension only activates when desktop app is running (user must explicitly start the game)
- Reading page content is passive (no modifications except cosmetic visual effects)
- No data leaves the user's machine except optional AI API calls configured by user
- Extension can be easily disabled when not playing

## Privacy Considerations

All permissions are used in accordance with the [Privacy Policy](./PRIVACY_POLICY.md):

- No tracking or analytics
- No remote servers (except optional user-configured Gemini API)
- Session-only data handling
- Transparent open-source code
