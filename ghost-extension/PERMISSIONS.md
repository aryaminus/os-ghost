# Permission Justification for OS Ghost Bridge

This document explains why each permission is required for the OS Ghost Bridge Chrome extension to function properly. Use this information when submitting to the Chrome Web Store.

## Required Permissions

### `nativeMessaging`

**Justification**: This permission is the **core functionality** of the extension. OS Ghost Bridge exists solely to connect Chrome browser with The OS Ghost desktop application (built with Tauri). Without native messaging, the extension cannot communicate with the desktop app and becomes non-functional.

**How it's used**:

- Send page navigation events to the desktop app
- Receive commands to inject visual effects
- Establish connection status monitoring

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
