<script lang="ts">
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
</script>

<SettingsSection
	title="Metadata Profiles"
	description="Control which metadata sources and fields are used when tagging music files."
>
	<EmptyState message="Metadata Profiles management will be implemented in Issue #462." />
</SettingsSection>
