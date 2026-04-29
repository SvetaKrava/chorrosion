<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError, login } from '$lib/api';
	import { handleLoginSuccess } from '$lib/auth';

	const DEFAULT_ERROR = 'Request failed. Please verify API server settings and try again.';

	let username = $state('');
	let password = $state('');
	let loginError = $state('');
	let busy = $state(false);

	function apiErrorMessage(error: unknown): string {
		if (error instanceof ApiError) {
			return error.message;
		}
		if (error instanceof Error) {
			return error.message;
		}
		return DEFAULT_ERROR;
	}

	async function onLoginSubmit(event: SubmitEvent): Promise<void> {
		event.preventDefault();
		loginError = '';
		busy = true;
		try {
			const response = await login(username.trim(), password);
			handleLoginSuccess(response, username);
			password = '';
			// Redirect to dashboard after successful login
			await goto('/dashboard');
		} catch (error) {
			loginError = apiErrorMessage(error);
		} finally {
			busy = false;
		}
	}
</script>

<main class="page">
	<section class="hero">
		<p class="eyebrow">Phase 11 UI Foundation</p>
		<h1>Chorrosion Control Deck</h1>
		<p>
			Editorial, realtime-first control surface for music library orchestration.
		</p>
	</section>

	<section class="panel">
		<h2>Sign In</h2>
		<form class="login-form" onsubmit={onLoginSubmit}>
			<label>
				<span>Username</span>
				<input bind:value={username} type="text" autocomplete="username" required />
			</label>
			<label>
				<span>Password</span>
				<input bind:value={password} type="password" autocomplete="current-password" required />
			</label>
			<button class="primary" type="submit" disabled={busy}>
				{busy ? 'Signing In…' : 'Sign In'}
			</button>
		</form>
		{#if loginError}
			<p class="error">{loginError}</p>
		{/if}
	</section>
</main>

<style>
	main {
		display: flex;
		flex-direction: column;
		gap: 2rem;
		max-width: 600px;
		margin: 0 auto;
		padding: 2rem;
	}

	.hero {
		text-align: center;
	}

	.eyebrow {
		font-size: 0.875rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--text-secondary);
		margin: 0 0 0.5rem 0;
	}

	h1 {
		margin: 0 0 1rem 0;
		font-size: 2.5rem;
		line-height: 1.2;
	}

	.hero > p {
		margin: 0;
		color: var(--text-secondary);
		font-size: 1.125rem;
	}

	.panel {
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 0.5rem;
		padding: 2rem;
	}

	.panel h2 {
		margin: 0 0 1.5rem 0;
	}

	.login-form {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		margin-bottom: 1rem;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	label > span {
		font-weight: 500;
		font-size: 0.875rem;
	}

	input {
		padding: 0.75rem;
		border: 1px solid var(--border-color);
		border-radius: 0.25rem;
		font-size: 1rem;
		background: var(--bg-primary);
		color: var(--text-primary);
	}

	input:focus {
		outline: none;
		border-color: var(--accent);
		box-shadow: 0 0 0 2px rgba(var(--accent-rgb), 0.1);
	}

	button {
		padding: 0.75rem;
		border: none;
		border-radius: 0.25rem;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	button.primary {
		background: var(--accent);
		color: white;
	}

	button.primary:hover:not(:disabled) {
		opacity: 0.9;
		transform: translateY(-1px);
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.error {
		color: var(--error);
		margin: 0;
		padding: 0.75rem;
		background: rgba(var(--error-rgb), 0.1);
		border-radius: 0.25rem;
	}
</style>
