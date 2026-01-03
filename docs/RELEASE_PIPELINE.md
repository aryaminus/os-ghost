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

| Secret Name | Description | Required |
|---|---|---|
| `GH_OWNER_TOKEN` | **Critical.** Personal Access Token (PAT) with `repo` and `workflow` scopes. Allows `bump_version` to trigger `release`. | Yes |
| `APPLE_CERTIFICATE` | Base64 encoded `.p12` file (Output of export script). | For macOS signing |
| `APPLE_CERTIFICATE_PASSWORD` | Password to decrypt the p12. | For macOS signing |
| `APPLE_SIGNING_IDENTITY` | The name of the cert (e.g., `Apple Distribution: ...`). | For macOS signing |
| `KEYCHAIN_PASSWORD` | Password for temporary keychain used during signing. | For macOS signing |
| `APPLE_API_ISSUER` | For Notarization. | Optional |
| `APPLE_API_KEY` | For Notarization. | Optional |
| `TAURI_PRIVATE_KEY` | Private key for signing updater artifacts. | For updater feature |
| `TAURI_KEY_PASSWORD` | Password for the Tauri signing key. | For updater feature |
| `AZURE_CLIENT_ID` | Azure credentials for Windows code signing. | For Windows signing |
| `AZURE_CLIENT_SECRET` | Azure credentials for Windows code signing. | For Windows signing |
| `AZURE_TENANT_ID` | Azure credentials for Windows code signing. | For Windows signing |

---

## 📦 Tauri Updater Configuration

The Tauri updater feature allows the app to automatically update itself. However, it requires proper signing configuration.

### Current Status

**Updater artifacts are currently DISABLED** (`createUpdaterArtifacts: false` in `src-tauri/tauri.conf.json`).

This was done to allow builds to succeed without requiring the `TAURI_PRIVATE_KEY` and `TAURI_KEY_PASSWORD` secrets to be properly configured.

### Enabling the Updater

To re-enable automatic updates:

1. **Generate signing keys**:
   ```bash
   # Install Tauri CLI
   npm install -g @tauri-apps/cli
   
   # Generate keypair
   tauri signer generate -w ~/.tauri/myapp.key
   ```
   
   This will output a public key and create a private key file.

2. **Add GitHub Secrets**:
   - `TAURI_PRIVATE_KEY`: Contents of the private key file
   - `TAURI_KEY_PASSWORD`: Password you set during key generation

3. **Update configuration**:
   - Set `createUpdaterArtifacts: true` in `src-tauri/tauri.conf.json`
   - The public key is already configured in the `updater.pubkey` field

4. **Update workflow** (`.github/workflows/release.yml`):
   - Add back the environment variables:
     ```yaml
     TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
     TAURI_SIGNING_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
     ```

### Why It Was Disabled

Without proper signing keys, the build process would fail with:
```
failed to decode secret key: incorrect updater private key password: Wrong password for that key
```

By disabling updater artifacts, the application can still be built and released, just without the automatic update capability.
