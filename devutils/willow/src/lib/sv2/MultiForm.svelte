<script lang="ts">
	import type { Event, FormElement, FormAction, FormData } from '../events.parse';
	import Button from '$lib/Button.svelte';
    import { auth } from '$lib/auth.svelte';
    import { mps } from '$lib/mainpagestate.svelte';
    import { encode } from '$lib/msyscall/khronosvalue';
    import FormInner from './FormInner.svelte';

	let { template, id, reorderable, forms, actions }: {
		template: string,
		id: string, 
		reorderable: boolean
		forms: FormElement[],
		actions: FormAction[],
	} = $props();

	let formOrder = $state<string[] | null>(null);

	const sortedForms = $derived.by(() => {
		const fd = mps.state.settings[template].formdata[id]
		if (!formOrder) return fd;
		
		// Map the order to actual form objects
		const ordered = formOrder.map(fid => fd.find(f => f.id === fid)).filter(Boolean) as FormData[];
		// Include any forms that might have been added but aren't in the order list yet
		const remaining = fd.filter(f => !formOrder!.includes(f.id));
		
		return [...ordered, ...remaining];
	});

	function startReordering() {
		// Initialize the order from the currently displayed sequence
		formOrder = sortedForms.map(f => f.id);
	}

	function move(index: number, direction: 'up' | 'down') {
		if (!formOrder) return;
		const newIndex = direction === 'up' ? index - 1 : index + 1;
		if (newIndex < 0 || newIndex >= formOrder.length) return;

		const next = [...formOrder];
		[next[index], next[newIndex]] = [next[newIndex], next[index]];
		formOrder = next;
	}

	const saveOrder = async () => {
        if(!formOrder) throw new Error("No form order found")
		const sve: Event = {
            type: "formset_reorder",
			__tloop_template_id: template,
            id,
            list: formOrder
        }

        if (!mps.state.selectedGuild) throw new Error("Guild not selected")
        await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode(sve))
		formOrder = null
	}

	const cancelReordering = () => {
		formOrder = null; // Revert to original order
	}
</script>

<div class="flex flex-col gap-6 w-full">
	{#if reorderable}
		<div class="flex justify-end gap-2 px-1">
			{#if !formOrder}
				<Button onclick={startReordering}>
					<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" class="inline mr-1"><path d="m16 8-4-4-4 4"/><path d="m16 16-4 4-4-4"/></svg>
					Reorder Forms
				</Button>
			{:else}
				<Button onclick={cancelReordering}>
					Cancel
				</Button>
				<Button onclick={async () => {
                    try {
                        await saveOrder()
                    } catch (err) {
                        formOrder = null // reset order back to previous
                        alert(err?.toString() || "Unknown error sending action")
                    }
                }}>
					Save Order
				</Button>
			{/if}
		</div>
	{/if}

	<div class="flex flex-col gap-4">
		{#if formOrder}
			{#each sortedForms as form, i (form.id)}
				<section 
					class="p-5 border rounded-3xl bg-white shadow-sm animate-in fade-in slide-in-from-bottom-2 duration-300 relative group transition-all ring-2 ring-blue-500/20 border-blue-500/50 scale-[0.99] translate-x-2"
					aria-labelledby="form-title-{form.id}"
				>
					<div class="absolute -left-10 top-1/2 -translate-y-1/2 flex flex-col gap-1.5 animate-in slide-in-from-right-2 duration-200">
						<button 
							onclick={() => move(i, 'up')}
							disabled={i === 0}
							class="p-2 bg-white border border-gray-200 rounded-xl hover:bg-gray-50 hover:text-blue-600 shadow-md transition-all active:scale-90 disabled:opacity-30 disabled:cursor-not-allowed"
							title="Move Up"
						>
							<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m18 15-6-6-6 6"/></svg>
						</button>
						<button 
							onclick={() => move(i, 'down')}
							disabled={i === sortedForms.length - 1}
							class="p-2 bg-white border border-gray-200 rounded-xl hover:bg-gray-50 hover:text-blue-600 shadow-md transition-all active:scale-90 disabled:opacity-30 disabled:cursor-not-allowed"
							title="Move Down"
						>
							<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
						</button>
					</div>

					<header class="flex items-center justify-between mb-6 border-b border-gray-50 pb-3">
						<div class="flex flex-col gap-0.5">
							<h3 id="form-title-{form.id}" class="text-sm font-black uppercase tracking-widest text-gray-400">
								{form.title || 'Untitled Form'}
							</h3>
							<code class="text-[10px] text-gray-400 font-mono">ID: {form.id}</code>
						</div>
						
						<span class="flex items-center gap-1.5 px-2.5 py-1 bg-blue-600 text-white animate-pulse rounded-full text-[10px] font-black uppercase tracking-wider border border-blue-100/50 shadow-sm transition-colors">
							<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><path d="m16 8-4-4-4 4"/><path d="m16 16-4 4-4-4"/></svg>
							Reordering...
						</span>
					</header>
				</section>
			{/each}
		{:else}
			{#each mps.state.settings[template].formdata[id] as form, i (form.id)}
				<section 
					class="p-5 border rounded-3xl bg-white shadow-sm border-gray-200 animate-in fade-in slide-in-from-bottom-2 duration-300 relative group transition-all"
					aria-labelledby="form-title-{form.id}"
				>
					<header class="flex items-center justify-between mb-6 border-b border-gray-50 pb-3">
						<div class="flex flex-col gap-0.5">
							<h3 id="form-title-{form.id}" class="text-sm font-black uppercase tracking-widest text-gray-400">
								{form.title || 'Untitled Form'}
							</h3>
							<code class="text-[10px] text-gray-400 font-mono">ID: {form.id}</code>
						</div>
											</header>

					<div class="flex flex-col gap-5 transition-all">
						<FormInner template={template} form={forms} formid={form.id} formidx={i} formsetid={id} actions={actions}/>
					</div>
				</section>
			{/each}
		{/if}
	</div>
</div>
