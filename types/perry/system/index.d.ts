// Type declarations for perry/system — Perry's platform & system APIs
// These types are auto-written by `perry init` / `perry types` so IDEs
// and tsc can resolve `import { ... } from "perry/system"`.

// ---------------------------------------------------------------------------
// Theme & Device
// ---------------------------------------------------------------------------

/** Returns true if the system is in dark mode. */
export function isDarkMode(): boolean;

/** Returns the device idiom (e.g. "phone", "pad", "mac", "tv"). */
export function getDeviceIdiom(): string;

/** Returns the device model identifier (e.g. "iPhone13,4"). */
export function getDeviceModel(): string;

// ---------------------------------------------------------------------------
// URL
// ---------------------------------------------------------------------------

/** Open a URL in the default browser or system handler. */
export function openURL(url: string): void;

// ---------------------------------------------------------------------------
// Keychain (secure credential storage)
// ---------------------------------------------------------------------------

/** Save a value to the system keychain. */
export function keychainSave(key: string, value: string): void;

/** Retrieve a value from the system keychain. */
export function keychainGet(key: string): string;

/** Delete a value from the system keychain. */
export function keychainDelete(key: string): void;

// ---------------------------------------------------------------------------
// User Preferences (persistent key-value storage)
// ---------------------------------------------------------------------------

/** Read a numeric preference value. */
export function preferencesGet(key: string): number;

/** Write a numeric preference value. */
export function preferencesSet(key: string, value: number): void;

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

/** Send a local notification. */
export function notificationSend(title: string, body: string): void;

// ---------------------------------------------------------------------------
// Audio input
// ---------------------------------------------------------------------------

/** Start audio capture. Returns 1 on success, 0 on failure. */
export function audioStart(): number;

/** Stop audio capture. */
export function audioStop(): void;

/** Get the current audio input level (0-1). */
export function audioGetLevel(): number;

/** Get the peak audio input level (0-1). */
export function audioGetPeak(): number;

/** Get waveform data with the given number of samples. */
export function audioGetWaveform(sampleCount: number): number;
