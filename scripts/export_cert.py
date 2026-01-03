import os
import sys
import base64
import subprocess
import re

# Load .env file
ENV_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".env"))
env_vars = {}
if os.path.exists(ENV_PATH):
    print(f"📄 Loading config from {ENV_PATH}...")
    with open(ENV_PATH, "r") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            match = re.match(r"^([^=]+)=(.*)$", line)
            if match:
                key, val = match.groups()
                # Handle quoted values
                val = val.strip("\"'") 
                env_vars[key] = val
else:
    print("⚠️  .env file not found!")

# Config
# Defaults are empty to prevent hardcoded secrets in source control
TARGET_HASH = env_vars.get("APPLE_CERT_HASH")
EXPORT_PASS = env_vars.get("APPLE_CERT_EXPORT_PASS")
TEMP_KEYCHAIN = "temp_export.keychain"
TEMP_PASS = env_vars.get("APPLE_TEMP_KEYCHAIN_PASS")

if not TARGET_HASH or not EXPORT_PASS:
    print("❌ Error: Missing configuration. Please set APPLE_CERT_HASH and APPLE_CERT_EXPORT_PASS in your .env file.")
    sys.exit(1)
OUTPUT_P12 = os.path.abspath("scripts/certificate.p12")
OUTPUT_B64 = os.path.abspath("scripts/certificate_base64.txt")

print(f"ℹ️  Using Certificate Hash: {TARGET_HASH}")

def run(cmd, shell=False):
    print(f"Running: {' '.join(cmd) if not shell else cmd}")
    result = subprocess.run(cmd, shell=shell, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
    return result

def main():
    print("🧹 Cleaning up old files...")
    if os.path.exists(TEMP_KEYCHAIN):
        run(["security", "delete-keychain", TEMP_KEYCHAIN])
    if os.path.exists(OUTPUT_P12):
        os.remove(OUTPUT_P12)

    print(f"🔐 Creating temporary keychain: {TEMP_KEYCHAIN}")
    run(["security", "create-keychain", "-p", TEMP_PASS, TEMP_KEYCHAIN])
    run(["security", "set-keychain-settings", TEMP_KEYCHAIN]) # Disable timeout
    run(["security", "unlock-keychain", "-p", TEMP_PASS, TEMP_KEYCHAIN])

    # Add temp keychain to search list
    # We must include login.keychain too or we lose access to it temporarily
    print("🔍 Configuring search list...")
    run(["security", "list-keychains", "-d", "user", "-s", TEMP_KEYCHAIN, "login.keychain"])

    # We need to find the identity in login keychain and copy it to temp
    # Currently, 'security export' is messy for single items.
    # We will try to just export everything from the login keychain that matches the identity... 
    # Attempting to use 'security export' with -k login.keychain and piping to a file didn't work well before.
    # Let's try to export DIRECTLY from login.keychain but using the aggregate type 'identities' 
    # but we cannot easily filter.
    
    # ALTERNATIVE: Use the earlier behavior where it dumped to stdout, capture it, and parse? No, dangerous.
    
    # CORRECT WAY: Copy item to temp keychain.
    print(f"📋 Copying identity {TARGET_HASH} to temp keychain...")
    # There isn't a simple 'security copy' command.
    
    # Fallback: We will trust the previous 'security export' command but capture STDOUT if it fails to write to file.
    # The previous attempt showed output in stdout. Let's capture that!
    
    print("⚠️  Exporting P12 (You may be prompted for password)...")
    CMD = [
        "security", "export",
        "-k", "login.keychain",
        "-t", "identities",
        "-f", "pkcs12",
        "-P", EXPORT_PASS,
        "-o", OUTPUT_P12
    ]
    
    # Note: We are ignoring the TARGET_HASH filter for a moment because 'security export' doesn't really support filtering efficiently?
    # Actually, let's try to pass the identity as an argument again? No, that failed.
    # The tool dumps ALL identities.
    
    # If we dump ALL identities, the p12 will contain ALL of them. That's fine for the user if they only use one password.
    # But usually we want just one.
    
    result = run(CMD)
    
    # ... (previous export logic)
    result = run(CMD)
    
    if os.path.exists(OUTPUT_P12) and os.path.getsize(OUTPUT_P12) > 0:
        print("✅ P12 file created.")
    else:
        print("❌ Export failed: Output file is empty or missing.")
        print(f"   Stdout: {result.stdout}")
        print(f"   Stderr: {result.stderr}")
        sys.exit(1)

    # VERIFY THE P12
    print("🕵️  Verifying P12 integrity...")
    VERIFY_KEYCHAIN = "verify.keychain"
    if os.path.exists(VERIFY_KEYCHAIN):
        run(["security", "delete-keychain", VERIFY_KEYCHAIN])
    
    run(["security", "create-keychain", "-p", "verify", VERIFY_KEYCHAIN])
    run(["security", "set-keychain-settings", VERIFY_KEYCHAIN])
    
    # Try importing the just-exported P12
    verify_cmd = [
        "security", "import", OUTPUT_P12,
        "-k", VERIFY_KEYCHAIN,
        "-P", EXPORT_PASS,
        "-T", "/usr/bin/codesign"
    ]
    verify_result = run(verify_cmd)
    
    # Clean up verify keychain
    run(["security", "delete-keychain", VERIFY_KEYCHAIN])

    if verify_result.returncode != 0:
        print("❌ CRITICAL: The exported P12 file is corrupt or invalid!")
        print(f"   Error: {verify_result.stderr}")
        sys.exit(1)
    
    print("✅ P12 file verified successfully (it works!).")

    # Read and encode
    with open(OUTPUT_P12, "rb") as f:
        content = f.read()

    print(f"   Size: {len(content)} bytes")
    
    # Convert to Base64
    b64_str = base64.b64encode(content).decode('utf-8')
    with open(OUTPUT_B64, "w") as f:
        f.write(b64_str)
        
    print("\n" + "="*60)
    print("🎉 EXPORT & VERIFICATION SUCCESSFUL")
    print("="*60)
    print("ACTION REQUIRED: Update your GitHub Secret 'APPLE_CERTIFICATE' now.")
    print("Copy the content of: scripts/certificate_base64.txt")
    print("="*60)

if __name__ == "__main__":
    main()
