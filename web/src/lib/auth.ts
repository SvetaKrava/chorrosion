// Authentication state management and utilities
import { writable } from 'svelte/store';
import type { FormsLoginResponse } from './types.js';

export interface AuthState {
  isAuthenticated: boolean;
  username: string | null;
  token: string | null;
}

export const authStore = writable<AuthState>({
  isAuthenticated: false,
  username: null,
  token: null
});

// Initialize auth state from session storage (persists across page reloads)
export function initializeAuth(): void {
  if (typeof sessionStorage === 'undefined') return;
  const stored = sessionStorage.getItem('auth_state');
  if (!stored) return;
  try {
    const parsed = JSON.parse(stored);
    // Validate shape and types before trusting the stored value
    if (
      parsed !== null &&
      typeof parsed === 'object' &&
      typeof parsed.isAuthenticated === 'boolean' &&
      (parsed.username === null || typeof parsed.username === 'string') &&
      (parsed.token === null || typeof parsed.token === 'string')
    ) {
      authStore.set(parsed as AuthState);
    } else {
      console.warn('Stored auth state has invalid shape; clearing.');
      sessionStorage.removeItem('auth_state');
      authStore.set({ isAuthenticated: false, username: null, token: null });
    }
  } catch (e) {
    console.error('Failed to parse stored auth state:', e);
    sessionStorage.removeItem('auth_state');
    authStore.set({ isAuthenticated: false, username: null, token: null });
  }
}

// Update auth state and persist to session storage
export function setAuthState(state: AuthState): void {
  authStore.set(state);
  if (typeof sessionStorage !== 'undefined') {
    sessionStorage.setItem('auth_state', JSON.stringify(state));
  }
}

// Clear auth state
export function clearAuthState(): void {
  authStore.set({
    isAuthenticated: false,
    username: null,
    token: null
  });
  if (typeof sessionStorage !== 'undefined') {
    sessionStorage.removeItem('auth_state');
  }
}

// Handle successful login
export function handleLoginSuccess(response: FormsLoginResponse, username: string): void {
	setAuthState({
		isAuthenticated: true,
		username,
		token: null // Forms auth uses session cookies, not tokens
	});
}

// Handle logout
export function handleLogout(): void {
  clearAuthState();
}
