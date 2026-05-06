/**
 * Unsaved guard store.
 *
 * Usage:
 *   import { createUnsavedGuard } from '$lib/stores/unsavedGuard';
 *
 *   // In a settings page component:
 *   const guard = createUnsavedGuard(() => { showConfirmDialog = true; });
 *
 *   // Mark dirty when a field changes:
 *   guard.markDirty();
 *
 *   // Clear after successful save:
 *   guard.markClean();
 *
 *   // Check state imperatively (e.g. to disable a Save button):
 *   if (guard.isDirty) { ... }
 *
 *   // Navigation is blocked automatically via beforeNavigate; the optional
 *   // onNavigateBlocked callback lets you open a confirm dialog in response.
 */

import { beforeNavigate } from '$app/navigation';

export interface UnsavedGuard {
	readonly isDirty: boolean;
	markDirty: () => void;
	markClean: () => void;
	/** Returns true if navigation should proceed (not dirty or user confirmed). */
	confirmNavigation: () => boolean;
}

/**
 * Create a per-component unsaved guard.
 * Call this once at component initialisation (not inside a reactive block).
 *
 * @param onNavigateBlocked - optional callback invoked when navigation is blocked;
 *   use this to open your ConfirmDialog. Resolve by calling markClean() + goto().
 */
export function createUnsavedGuard(onNavigateBlocked?: () => void): UnsavedGuard {
	let dirty = false;

	beforeNavigate(({ cancel }) => {
		if (dirty) {
			cancel();
			if (onNavigateBlocked) {
				onNavigateBlocked();
			}
		}
	});

	return {
		get isDirty() {
			return dirty;
		},
		markDirty() {
			dirty = true;
		},
		markClean() {
			dirty = false;
		},
		confirmNavigation() {
			return !dirty;
		}
	};
}
