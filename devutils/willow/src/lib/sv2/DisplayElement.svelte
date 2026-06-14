<script lang="ts">
    import ErrorBox from '$lib/ErrorBox.svelte';
    import type { DisplayElement } from '../events.parse';

	let { el = $bindable() }: { el: DisplayElement } = $props();
</script>

{#if el.type == "Header"}
    <h4 class="text-md font-semibold mb-2">{el.text}</h4>
{:else if el.type == "Paragraph"}
    <p class="mb-2">{el.text}</p>
{:else if el.type == "Error"}
    <ErrorBox error={el.error} />
{/if}