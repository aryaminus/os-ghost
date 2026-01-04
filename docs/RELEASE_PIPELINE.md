# 🚀 Release Pipeline & Code Signing

This repository uses a fully automated CI/CD pipeline to version, build, sign, and release the application.

## 🔄 The Workflow

The release process is triggered automatically by git events.

### 1. Auto-Versioning (`bump-version.yml`)

* **Trigger**: Push to `main` branch.
* **Action**:
    1. **Detects which files changed** (app vs extension).
    2. **If app files changed**:
        * Bumps version in `package.json`, `src-tauri/tauri.conf.json`, `Cargo.toml`
        * Commits: `chore(release): bump version to X.Y.Z`
        * Pushes tag: `app-vX.Y.Z`
    3. **If extension files changed**:
        * Bumps version in `ghost-extension/manifest.json`
        * Commits: `chore(extension): bump version to X.Y.Z`
        * Pushes tag: `ext-vX.Y.Z`
* **Note**: Uses `GH_OWNER_TOKEN` to bypass GitHub's workflow recursion restrictions.

### 2. Desktop App Release (`app-release.yml`)

* **Trigger**: Push of a tag starting with `app-v*`.
* **Action**:
    1. Builds the application for **macOS (Intel & Silicon)**, **Windows**, and **Linux**.
    2. Signs updater artifacts with Tauri keys.
    3. Creates **GitHub Release** with platform binaries.

### 3. Chrome Extension Release (`extension-release.yml`)

* **Trigger**: Push of a tag starting with `ext-v*`.
* **Action**:
    1. Packages the extension into a zip file.
    2. **Publishes to Chrome Web Store** (requires CWS API credentials).
    3. Creates **GitHub Release** with extension zip.

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
