<script lang="ts">
	import { page } from '$app/stores';

	let { children } = $props();

	const subNavItems = [
		{ label: 'Download Clients', href: '/settings/download-clients' },
		{ label: 'Indexers', href: '/settings/indexers' },
		{ label: 'Quality Profiles', href: '/settings/quality-profiles' },
		{ label: 'Metadata Profiles', href: '/settings/metadata-profiles' },
		{ label: 'Appearance', href: '/settings/appearance' }
	];

	function isActive(href: string): boolean {
		return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
	}
</script>

<div class="settings-shell">
	<aside class="settings-sidebar" aria-label="Settings navigation">
		<h2 class="settings-title">Settings</h2>
		<nav>
			<ul class="settings-nav" role="list">
				{#each subNavItems as item}
					<li>
						<a
							href={item.href}
							class="settings-nav-link"
							class:active={isActive(item.href)}
							aria-current={isActive(item.href) ? 'page' : undefined}
						>
							{item.label}
						</a>
					</li>
				{/each}
			</ul>
		</nav>
	</aside>

	<div class="settings-content">
		{@render children()}
	</div>
</div>

<style>
	.settings-shell {
		display: grid;
		grid-template-columns: 220px 1fr;
		gap: 0;
		max-width: 1200px;
		margin: 0 auto;
		min-height: calc(100vh - 8rem);
	}

	.settings-sidebar {
		border-right: 1px solid var(--border-color);
		padding: 1.5rem 0;
	}

	.settings-title {
		font-size: 0.75rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-secondary);
		margin: 0 0 0.75rem 0;
		padding: 0 1.25rem;
	}

	.settings-nav {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.settings-nav-link {
		display: block;
		padding: 0.6rem 1.25rem;
		color: var(--text-secondary);
		text-decoration: none;
		font-size: 0.9rem;
		font-weight: 500;
		border-left: 2px solid transparent;
		transition: all 0.15s;
	}

	.settings-nav-link:hover {
		color: var(--text-primary);
		background: rgba(var(--accent-rgb), 0.03);
	}

	.settings-nav-link.active {
		color: var(--accent);
		border-left-color: var(--accent);
		background: rgba(var(--accent-rgb), 0.06);
	}

	.settings-content {
		padding: 2rem 2.5rem;
		min-width: 0;
	}

	@media (max-width: 700px) {
		.settings-shell {
			grid-template-columns: 1fr;
		}

		.settings-sidebar {
			border-right: none;
			border-bottom: 1px solid var(--border-color);
			padding: 1rem 0 0;
		}

		.settings-nav {
			display: flex;
			overflow-x: auto;
			gap: 0;
		}

		.settings-nav-link {
			border-left: none;
			border-bottom: 2px solid transparent;
			white-space: nowrap;
		}

		.settings-nav-link.active {
			border-left-color: transparent;
			border-bottom-color: var(--accent);
		}

		.settings-content {
			padding: 1.5rem 1rem;
		}
	}
</style>
