# Privacy Policy for OS Ghost Bridge Chrome Extension

**Last Updated:** January 3, 2026

## Overview

The OS Ghost Bridge extension ("Extension") is a companion tool for The OS Ghost desktop game. This privacy policy explains what data the Extension collects, how it is used, and your rights regarding this data.

## Data Collection

The Extension collects the following data from your browser:

| Data Type | What We Collect | Purpose |
|-----------|-----------------|---------|
| **Page URLs** | URLs of websites you visit | Detect puzzle solutions in the game |
| **Page Titles** | Titles of visited pages | Provide context for game puzzles |
| **Page Content** | Visible text on pages (first 5,000 characters) | Enable AI-powered puzzle generation |
| **Browser History (Recent)** | A limited set of recent URLs/titles/visit metadata | Provide immediate context for puzzle generation |
| **Top Sites** | A short list of most visited sites | Provide additional context for puzzle generation |

## How Data Is Used

All collected data is used exclusively for gameplay purposes:

1. **Local Processing**: Data is sent to the OS Ghost desktop app running on your computer via Chrome's native messaging API
2. **AI Analysis (Optional)**: If you have configured a Gemini API key in the desktop app, page content may be sent to Google's Gemini API for puzzle generation
3. **No External Servers**: We do not operate any servers. Data is never transmitted to any remote servers we control

## Data Storage

- **No Persistent Storage**: The Extension does not permanently store any browsing data
- **Session Only**: Data exists only during active gameplay sessions
- **Local Storage**: Only connection status (connected/disconnected) is stored locally using Chrome's storage API

## Third-Party Services

When using AI-powered features in the desktop app:

- Google Gemini API may receive page content for analysis
- Google's [Privacy Policy](https://policies.google.com/privacy) applies to this processing
- You can disable AI features by not providing a Gemini API key

## Permissions Explained

| Permission | Why We Need It |
|------------|----------------|
| `nativeMessaging` | Core functionality - communicate with OS Ghost desktop app |
| `storage` | Store connection status locally (connected/disconnected) |
| `history` | Read recent browsing history for puzzle context |
| `topSites` | Read top visited sites for puzzle context |
| `host_permissions: <all_urls>` | Allow content script to read visible text on any site you visit while playing |

## User Control

- **Disable Anytime**: You can disable or remove the Extension at any time via `chrome://extensions`
- **Desktop App Required**: Data collection only occurs when the OS Ghost desktop app is running
- **No Tracking**: We do not use any analytics or tracking services

## Children's Privacy

This Extension is not intended for children under 13 years of age.

## Changes to This Policy

We may update this privacy policy. Changes will be reflected in the "Last Updated" date above.

## Contact

For questions about this privacy policy or the Extension:

- **GitHub Issues**: [github.com/aryaminus/os-ghost/issues](https://github.com/aryaminus/os-ghost/issues)
- **Repository**: [github.com/aryaminus/os-ghost](https://github.com/aryaminus/os-ghost)

---

*This Extension is open source under the MIT License.*
