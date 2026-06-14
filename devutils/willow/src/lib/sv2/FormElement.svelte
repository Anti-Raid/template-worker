<script lang="ts">
    import TextBox from '$lib/TextBox.svelte';
    import type { FormElement } from '../events.parse';

	let { el = $bindable() }: { el: FormElement } = $props();
</script>

{#if el.type == "Text"}
    <TextBox id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={el.value} readonly={el.disabled} />
{/if}