// Root layout: initialize auth state only. Route guards live in (app)/+layout.ts.
import type { LayoutLoad } from './$types';
import { authStore, initializeAuth } from '$lib/auth';
import { get } from 'svelte/store';
import { browser } from '$app/environment';

// Disable SSR for this SPA — sessionStorage and auth guards are browser-only.
export const ssr = false;

export const load: LayoutLoad = async () => {
  if (browser) {
    initializeAuth();
  }

  const auth = get(authStore);

  return {
    isAuthenticated: auth.isAuthenticated,
    username: auth.username
  };
};
