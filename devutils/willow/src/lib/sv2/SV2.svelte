<script lang="ts">
    import type { Component } from '../events.parse';
    import DisplayElement from './DisplayElement.svelte';
    import Form from './MultiForm.svelte';
	import SV2 from "./SV2.svelte"

	let { comps }: { comps: Component[] } = $props();
</script>

{#each comps as comp}
	{#if comp.type == "DisplayElement"}
		<DisplayElement el={comp.element} />
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
	{:else if comp.type == "#Willow.MultiForm"}
		<Form id={comp.id} forms={comp.forms} reorderable={comp.reorderable} />
	{/if}
{/each}