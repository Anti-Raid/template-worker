<script lang="ts">
    import type { Component } from '../events.parse';
    import Form from './MultiForm.svelte';
	import SV2 from "./SV2.svelte"

	let { comps }: { comps: Component[] } = $props();
</script>

{#each comps as comp}
	{#if comp.type == "TextBlock"}
		{#if comp.style == "Header"}
			<h4 class="text-md font-semibold mb-2">{comp.text}</h4>
		{:else if comp.style == "Paragraph"}
			<p class="mb-2">{comp.text}</p>
		{/if}
	{:else if comp.type == "Section"}
		<section id={comp.id}>
			<details>
				<summary class="hover:cursor-pointer">
					<span class="text-lg font-semibold">{comp.title} ({comp.id})</span>
					<p class="mb-2">{comp.description}</p>
				</summary>
				<SV2 comps={comp.entries}/>
			</details>
		</section>
	{:else if comp.type == "Collapsible"}
		{#each comp.collapsibles as col}
			<details id={col.id}>
				<summary>
					<span class="text-lg font-semibold">{col.label} ({col.id})</span>
				</summary>
				<SV2 comps={col.entries}/>
			</details>
		{/each}
	{:else if comp.type == "#Willow.MultiForm"}
		<Form id={comp.id} forms={comp.forms} reorderable={comp.reorderable} />
	{/if}
{/each}