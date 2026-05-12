/**
 * Unsaved guard store.
 *
 * Usage:
 *   import { useUnsavedGuard } from '$lib/stores/unsavedGuard';
 *
 *   // In a settings page component:
 *   const guard = useUnsavedGuard(() => { showConfirmDialog = true; });
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
 *   // Call guard.confirmNavigation() after the user confirms to continue.
 */

import { beforeNavigate, goto } from '$app/navigation';

export interface UnsavedGuard {
	readonly isDirty: boolean;
	readonly hasPendingNavigation: boolean;
	markDirty: () => void;
	markClean: () => void;
	/** Continue the blocked navigation if one is pending. */
	confirmNavigation: () => Promise<boolean>;
	discardNavigation: () => void;
}

/**
 * Create a per-component unsaved guard.
 * Call this once at component initialisation (not inside a reactive block).
 *
 * @param onNavigateBlocked - optional callback invoked when navigation is blocked;
 *   use this to open your ConfirmDialog. Resolve by calling confirmNavigation().
 */
export function useUnsavedGuard(onNavigateBlocked?: () => void): UnsavedGuard {
	let dirty = false;
	let pendingHref: string | null = null;

	beforeNavigate(({ cancel, to }) => {
		if (dirty) {
			cancel();
			pendingHref = to?.url.href ?? null;
			if (onNavigateBlocked) {
				onNavigateBlocked();
			}
		}
	});

	return {
		get isDirty() {
			return dirty;
		},
		get hasPendingNavigation() {
			return pendingHref !== null;
		},
		markDirty() {
			dirty = true;
		},
		markClean() {
			dirty = false;
		},
		async confirmNavigation() {
			dirty = false;
			if (pendingHref) {
				const href = pendingHref;
				pendingHref = null;
				await goto(href);
			}
			return true;
		},
		discardNavigation() {
			pendingHref = null;
		}
	};
}

export function createUnsavedGuard(onNavigateBlocked?: () => void): UnsavedGuard {
	return useUnsavedGuard(onNavigateBlocked);
}
