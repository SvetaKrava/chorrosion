<script lang="ts">
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
</script>

<SettingsSection
	title="Download Clients"
	description="Configure download clients (NZBGet, SABnzbd, qBittorrent, etc.) used to fetch releases."
>
	<EmptyState message="Download Clients management will be implemented in Issue #459." />
</SettingsSection>
