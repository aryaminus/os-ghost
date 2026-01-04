# 🚀 Release Pipeline & Code Signing

This repository uses a fully automated CI/CD pipeline to version, build, sign, and release the application.

## 🔄 The Workflow

The release process is triggered automatically by git events.

### 1. Auto-Versioning (`bump_version.yml`)

* **Trigger**: Push to `main` branch.
* **Action**:
    1. Calculates the next patch version (e.g., `0.1.0` -> `0.1.1`).
    2. Updates `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
    3. Commits the change: `chore(release): bump version to ...`
    4. **Pushes a Tag**: `app-v0.1.x`
* **Note**: This uses a Personal Access Token (`GH_OWNER_TOKEN`) to bypass GitHub's restriction on recursively triggering workflows.

### 2. Build & Release (`release.yml`)

* **Trigger**: Push of a tag starting with `app-v*`.
* **Action**:
    1. Builds the application for **macOS (Intel & Silicon)**, **Windows**, and **Linux**.
    2. **Signs** the macOS build using the Apple Distribution Certificate (see below).
    3. Uploads artifacts to a new **GitHub Release**.

### 3. Chrome Extension (`extension-release.yml`)

* **Trigger**: Push to `main` that modifies `ghost-extension/**`.
* **Action**:
    1. Bumps the patch version in `manifest.json`.
    2. Commits the version bump.
    3. Packages the extension into a zip file.
    4. **Publishes to Chrome Web Store** (requires CWS API credentials).
    5. Creates a **GitHub Release** with tag `ext-vX.Y.Z`.

> **Note**: The workflow skips if the commit message contains `chore(extension):` to prevent infinite loops.

---

## 🔐 Code Signing Setup

Code signing is critical for macOS to allow the app to run without "damaged" errors.

### Local Development (Ad-Hoc)

Locally, we use **Ad-Hoc signing** (`"signingIdentity": "-"` in `tauri.conf.json`).

* **Allows**: Building and running on your own Mac (Apple Silicon).
* **Requires**: No certificates.

### CI/CD Production (Apple Distribution)

On GitHub Actions, we inject a real **Apple Distribution Certificate**.

#### 🛠️ Managing Certificates

We have a Python script to safely export your local Apple Certificate to the format GitHub needs without leaking files.

**Prerequisite:** You must have the *Apple Distribution: Core Software Integrated Inc.* certificate in your Keychain.

1. **Configure `.env`**:

    ```properties
    # .env
    APPLE_CERT_HASH=AAD722F36CE89BE6498EC8A20F21BD74D00FA1A7
    APPLE_CERT_EXPORT_PASS=create_a_strong_password
    APPLE_TEMP_KEYCHAIN_PASS=temp123
    ```

2. **Run the Export Script**:

    ```bash
    python3 scripts/export_cert.py
    ```

    *This will prompt for your system password to authorize the keychain export.*

3. **Upload Secrets**:
    The script will output 3 values. Add them to **GitHub Repo -> Settings -> Secrets -> Actions**:
    * `APPLE_CERTIFICATE`: (Base64 string)
    * `APPLE_CERTIFICATE_PASSWORD`: (Your export password)
    * `APPLE_SIGNING_IDENTITY`: (The identitifier string)

---

## 🔑 GitHub Secrets Reference

For the pipeline to work, these secrets must be present in the repository:

| Secret Name | Required | Description |
|---|---|---|
| `GH_OWNER_TOKEN` | ✅ Yes | Personal Access Token (PAT) with `repo` and `workflow` scopes. Used by `bump_version` to push commits and tags. |
| `TAURI_PRIVATE_KEY` | ✅ Yes | Base64-encoded private key for signing updater artifacts. Generated with `tauri signer generate`. |
| `TAURI_KEY_PASSWORD` | ✅ Yes | Password for the Tauri signing key. |
| `APPLE_CERTIFICATE` | ❌ No* | Base64 encoded `.p12` file (Output of export script). |
| `APPLE_CERTIFICATE_PASSWORD` | ❌ No* | Password to decrypt the p12. |
| `APPLE_SIGNING_IDENTITY` | ❌ No* | The name of the cert (e.g., `Apple Distribution: ...`). |
| `KEYCHAIN_PASSWORD` | ❌ No* | Password for CI keychain (any secure string). |
| `APPLE_API_ISSUER` | ❌ No | For Notarization. |
| `APPLE_API_KEY` | ❌ No | For Notarization. |
| `EXTENSION_ID` | ❌ No | Chrome Web Store extension ID (for auto-publish). |
| `CWS_CLIENT_ID` | ❌ No | Chrome Web Store API client ID. |
| `CWS_CLIENT_SECRET` | ❌ No | Chrome Web Store API client secret. |
| `CWS_REFRESH_TOKEN` | ❌ No | Chrome Web Store API refresh token. |

> **Note**: macOS code signing is currently **disabled** (using ad-hoc signing). To enable it, uncomment the Apple-related env vars in `release.yml` and ensure the secrets are configured.

> **Note**: Chrome Web Store publishing is optional. See [Chrome Web Store API guide](https://developer.chrome.com/docs/webstore/using_webstore_api/) for credentials.

---

## ⚙️ Repository Permissions

The `release.yml` workflow uses `GITHUB_TOKEN` to create releases. You **must** configure:

1. Go to **Settings → Actions → General**
2. Scroll to **"Workflow permissions"**
3. Select **"Read and write permissions"**
4. Click **Save**

Without this, the release workflow will fail with `Resource not accessible by integration`.

---

## 📝 Current Status

* **macOS Signing**: Ad-hoc (`signingIdentity: "-"` in `tauri.conf.json`)
* **Tauri Updater**: Enabled with signed artifacts
* **Notarization**: Disabled (requires Apple API credentials)
