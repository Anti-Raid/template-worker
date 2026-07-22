<script lang="ts">
    import type { Component, FormData } from '../events.parse';
    import DisplayElement from './DisplayElement.svelte';
    import Form from './MultiForm.svelte';
	import SV2 from "./SV2.svelte"

	let { template, comps }: { template: string, comps: Component[] } = $props();
</script>

{#each comps as comp}
	{#if comp.type == "DisplayElement"}
		<DisplayElement el={comp.element} />
	{:else if comp.type == "Section"}
		<section id={comp.id} class="mb-6">
			<details class="group bg-white rounded-2xl border border-gray-200 shadow-sm overflow-hidden" open>
				<summary class="flex flex-col cursor-pointer p-5 bg-gray-50/50 hover:bg-gray-50 transition-colors select-none">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-3">
							<svg class="w-5 h-5 text-gray-400 group-open:rotate-90 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/></svg>
							<span class="text-lg font-bold text-gray-900">{comp.title}</span>
							<span class="text-[10px] font-mono bg-gray-200 text-gray-600 px-2 py-0.5 rounded-md uppercase tracking-wider">{comp.id}</span>
						</div>
					</div>
					{#if comp.description}
						<p class="text-gray-500 text-sm mt-2 ml-8">{comp.description}</p>
					{/if}
				</summary>
				<div class="p-6 border-t border-gray-100 flex flex-col gap-8">
					<SV2 template={template} comps={comp.entries} />
				</div>
			</details>
		</section>
	{:else if comp.type == "FormSet"}
		<Form template={template} id={comp.id} forms={comp.forms} reorderable={comp.reorderable} actions={comp.actions} />
	{/if}
{/each}