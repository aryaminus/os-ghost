import fs from "fs";
import path from "path";
import { execSync } from "child_process";

// Get bump type (default to patch)
const bumpType = process.argv[2] || "patch";

const packagePath = path.resolve("package.json");
const tauriConfPath = path.resolve("src-tauri/tauri.conf.json");
const cargoPath = path.resolve("src-tauri/Cargo.toml");

// Read current version
const packageJson = JSON.parse(fs.readFileSync(packagePath, "utf-8"));
const currentVersion = packageJson.version;

console.log(`Current version: ${currentVersion}`);
console.log(`Bumping ${bumpType}...`);

// Use npm version to calculate next version cleanly
// --no-git-tag-version prevents npm from creating a tag immediately, we'll do it manually
execSync(`npm version ${bumpType} --no-git-tag-version`, { stdio: "inherit" });

// Read the new version
const newPackageJson = JSON.parse(fs.readFileSync(packagePath, "utf-8"));
const newVersion = newPackageJson.version;

console.log(`New version: ${newVersion}`);

// Update tauri.conf.json
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf-8"));
tauriConf.version = newVersion;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, "\t") + "\n");
console.log(`Updated tauri.conf.json to ${newVersion}`);

// Update Cargo.toml
let cargoContent = fs.readFileSync(cargoPath, "utf-8");
// Check if version matches before replacement to avoid accidents
if (cargoContent.includes(`version = "${currentVersion}"`)) {
	cargoContent = cargoContent.replace(
		`version = "${currentVersion}"`,
		`version = "${newVersion}"`
	);
	fs.writeFileSync(cargoPath, cargoContent);
	console.log(`Updated src-tauri/Cargo.toml to ${newVersion}`);

	// Update Cargo.lock
	console.log("Updating Cargo.lock...");
	execSync("cargo check", { cwd: "src-tauri", stdio: "inherit" });

	// Ensure package-lock.json is synced (although npm version usually handles it, this is a safety net)
	console.log("Ensuring package-lock.json is synced...");
	execSync("npm install", { stdio: "inherit" });
} else {
	console.warn(
		"Could not find exact version string in Cargo.toml, skipping update (manual check required?)"
	);
}

console.log("Version bump complete.");
