import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { authStore, initializeAuth, setAuthState, clearAuthState } from '$lib/auth';
import type { AuthState } from '$lib/auth';

const DEFAULT_STATE: AuthState = { isAuthenticated: false, username: null, token: null };

function resetStore() {
	authStore.set(DEFAULT_STATE);
}

describe('authStore initial state', () => {
	it('starts unauthenticated', () => {
		resetStore();
		const state = get(authStore);
		expect(state.isAuthenticated).toBe(false);
		expect(state.username).toBeNull();
		expect(state.token).toBeNull();
	});
});

describe('setAuthState', () => {
	beforeEach(() => {
		resetStore();
		sessionStorage.clear();
	});

	it('updates the store', () => {
		setAuthState({ isAuthenticated: true, username: 'alice', token: 'tok123' });
		const state = get(authStore);
		expect(state.isAuthenticated).toBe(true);
		expect(state.username).toBe('alice');
		expect(state.token).toBe('tok123');
	});

	it('persists to sessionStorage', () => {
		setAuthState({ isAuthenticated: true, username: 'alice', token: 'tok123' });
		const raw = sessionStorage.getItem('auth_state');
		expect(raw).not.toBeNull();
		const parsed = JSON.parse(raw!);
		expect(parsed.username).toBe('alice');
	});
});

describe('clearAuthState', () => {
	beforeEach(() => {
		sessionStorage.clear();
	});

	it('resets store to unauthenticated', () => {
		setAuthState({ isAuthenticated: true, username: 'alice', token: 'tok123' });
		clearAuthState();
		const state = get(authStore);
		expect(state).toEqual(DEFAULT_STATE);
	});

	it('removes sessionStorage entry', () => {
		setAuthState({ isAuthenticated: true, username: 'alice', token: 'tok123' });
		clearAuthState();
		expect(sessionStorage.getItem('auth_state')).toBeNull();
	});
});

describe('initializeAuth', () => {
	beforeEach(() => {
		resetStore();
		sessionStorage.clear();
	});

	it('loads a valid stored auth state', () => {
		const stored: AuthState = { isAuthenticated: true, username: 'bob', token: 'abc' };
		sessionStorage.setItem('auth_state', JSON.stringify(stored));
		initializeAuth();
		expect(get(authStore)).toEqual(stored);
	});

	it('ignores invalid JSON in sessionStorage', () => {
		sessionStorage.setItem('auth_state', 'not-valid-json');
		initializeAuth();
		expect(get(authStore)).toEqual(DEFAULT_STATE);
	});

	it('ignores stored values with wrong shape', () => {
		sessionStorage.setItem('auth_state', JSON.stringify({ isAuthenticated: 'yes' }));
		initializeAuth();
		expect(get(authStore)).toEqual(DEFAULT_STATE);
	});

	it('clears sessionStorage when shape is invalid', () => {
		sessionStorage.setItem('auth_state', JSON.stringify({ isAuthenticated: 'yes' }));
		initializeAuth();
		expect(sessionStorage.getItem('auth_state')).toBeNull();
	});

	it('does nothing when sessionStorage is empty', () => {
		initializeAuth();
		expect(get(authStore)).toEqual(DEFAULT_STATE);
	});
});
