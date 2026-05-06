<script lang="ts">
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
</script>

<SettingsSection
	title="Indexers"
	description="Configure Torznab, Newznab, and Gazelle indexers used to search for releases."
>
	<EmptyState message="Indexers management will be implemented in Issue #460." />
</SettingsSection>
