import type { LayoutLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { authStore, initializeAuth } from '$lib/auth';
import { get } from 'svelte/store';
import { browser } from '$app/environment';

export const load: LayoutLoad = async () => {
  // Initialize auth state from persistent storage (browser-only guard)
  if (browser) {
    initializeAuth();
  }

  const auth = get(authStore);

  // If not authenticated, redirect to login
  if (!auth.isAuthenticated) {
    throw redirect(303, '/');
  }

  return {
    isAuthenticated: auth.isAuthenticated,
    username: auth.username
  };
};
