<script lang="ts">
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
</script>

<SettingsSection
	title="Quality Profiles"
	description="Define which audio formats and bitrates are acceptable for automated downloading."
>
	<EmptyState message="Quality Profiles management will be implemented in Issue #461." />
</SettingsSection>
