<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { authStore, clearAuthState } from '$lib/auth';
	import { logout } from '$lib/api';

	let busy = $state(false);
	let { children } = $props();

	async function onLogout(): Promise<void> {
		busy = true;
		try {
			await logout();
		} catch {
			// Best-effort logout; local state must still reset
		}
		clearAuthState();
		try {
			await goto('/');
		} finally {
			busy = false;
		}
	}

	const navItems = [
		{ label: 'Dashboard', href: '/dashboard' },
		{ label: 'Artists', href: '/artists' },
		{ label: 'Albums', href: '/albums' },
		{ label: 'Appearance', href: '/appearance' }
	];

	function isActive(href: string): boolean {
		return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
	}
</script>

<div class="app-shell">
	<!-- Skip link for keyboard users -->
	<a href="#main-content" class="skip-link">Skip to main content</a>

	<header class="app-header">
		<div class="header-content">
			<h1 class="logo">Chorrosion</h1>
			<nav class="main-nav" aria-label="Main navigation">
				{#each navItems as item}
					<a
						href={item.href}
						class="nav-link"
						class:active={isActive(item.href)}
						data-sveltekit-noscroll
						aria-current={isActive(item.href) ? 'page' : undefined}
					>
						{item.label}
					</a>
				{/each}
			</nav>
			<div class="header-actions">
				<span class="username" aria-label="Logged in as {$authStore.username || 'User'}">{$authStore.username || 'User'}</span>
				<button class="logout-btn" onclick={onLogout} disabled={busy} aria-label={busy ? 'Logging Out' : 'Log Out'}>
					{busy ? 'Logging Out…' : 'Log Out'}
				</button>
			</div>
		</div>
	</header>

	<main class="app-main" id="main-content">
		{@render children()}
	</main>

	<footer class="app-footer">
		<p>Chorrosion Control Deck · Phase 11 UI</p>
	</footer>
</div>

<style>
	:global {
		body {
			margin: 0;
		}
	}

	.app-shell {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
		background: var(--bg-primary);
		color: var(--text-primary);
	}

	.app-header {
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border-color);
		position: sticky;
		top: 0;
		z-index: 100;
	}

	.header-content {
		max-width: 1400px;
		margin: 0 auto;
		padding: 0 1rem;
		display: flex;
		align-items: center;
		gap: 2rem;
		height: 4rem;
	}

	.logo {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 700;
		letter-spacing: -0.05em;
		white-space: nowrap;
		min-width: fit-content;
	}

	.main-nav {
		display: flex;
		gap: 0;
		flex: 1;
		align-items: center;
	}

	.nav-link {
		padding: 0.5rem 1rem;
		color: var(--text-secondary);
		text-decoration: none;
		font-weight: 500;
		font-size: 0.875rem;
		border-bottom: 2px solid transparent;
		transition: all 0.2s;
		white-space: nowrap;
	}

	.nav-link:hover {
		color: var(--text-primary);
		background: rgba(var(--accent-rgb), 0.05);
	}

	.nav-link.active {
		color: var(--accent);
		border-bottom-color: var(--accent);
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: 1.5rem;
		margin-left: auto;
	}

	.username {
		font-size: 0.875rem;
		color: var(--text-secondary);
	}

	.logout-btn {
		padding: 0.5rem 1rem;
		background: none;
		border: 1px solid var(--border-color);
		color: var(--text-primary);
		border-radius: 0.25rem;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.logout-btn:hover:not(:disabled) {
		background: rgba(var(--error-rgb), 0.1);
		border-color: var(--error);
		color: var(--error);
	}

	.logout-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.app-main {
		flex: 1;
		max-width: 1400px;
		width: 100%;
		margin: 0 auto;
		padding: 2rem 1rem;
	}

	.app-footer {
		border-top: 1px solid var(--border-color);
		padding: 1.5rem;
		text-align: center;
		color: var(--text-secondary);
		font-size: 0.75rem;
		background: var(--bg-secondary);
	}

	.app-footer p {
		margin: 0;
	}

	/* Mobile responsive */
	@media (max-width: 640px) {
		.header-content {
			gap: 1rem;
			height: auto;
			padding: 0.75rem 1rem;
			flex-wrap: wrap;
		}

		.logo {
			font-size: 1rem;
		}

		.main-nav {
			width: 100%;
			order: 3;
			margin-top: 0.5rem;
		}

		.nav-link {
			flex: 1;
			text-align: center;
			padding: 0.75rem 0.5rem;
			font-size: 0.75rem;
		}

		.header-actions {
			gap: 0.75rem;
			flex: 1;
		}

		.username {
			display: none;
		}

		.logout-btn {
			padding: 0.5rem 0.75rem;
			font-size: 0.75rem;
		}

		.app-main {
			padding: 1rem;
		}
	}

	/* Skip link for keyboard navigation */
	.skip-link {
		position: absolute;
		top: -40px;
		left: 0;
		background: var(--accent);
		color: white;
		padding: 8px;
		text-decoration: none;
		z-index: 200;
	}

	.skip-link:focus {
		top: 0;
	}
</style>
