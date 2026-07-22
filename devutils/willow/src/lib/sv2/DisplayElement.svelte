<script lang="ts">
    import ErrorBox from '$lib/ErrorBox.svelte';
    import type { DisplayElement } from '../events.parse';

	let { el = $bindable() }: { el: DisplayElement } = $props();
</script>

{#if el.type == "Header"}
    <div class="flex items-center gap-4 mb-5 mt-8 first:mt-0">
        <h2 class="text-lg font-bold text-gray-700 tracking-tight whitespace-nowrap">{el.text}</h2>
        <div class="h-px bg-gray-200 flex-1"></div>
    </div>
{:else if el.type == "Paragraph"}
    <p class="text-gray-600 mb-6 text-lg">{el.text}</p>
{:else if el.type == "Error"}
    <ErrorBox error={el.error} />
{/if}