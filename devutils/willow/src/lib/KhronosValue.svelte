<script lang="ts">
	import type { RawKhronosValue } from './msyscall/khronosvalue';
	import Select from './Select.svelte';
	import TextBox from './TextBox.svelte';
	import Checkbox from './Checkbox.svelte';
	import Button from './Button.svelte';
	import Number from './Number.svelte';
    import Self  from './KhronosValue.svelte'

	let {
        id,
		value = $bindable(),
		label = '',
		depth = 0
	}: { id: string, value: RawKhronosValue; label?: string; depth?: number } = $props();

	const types = [
		'Text', 'Integer', 'Int64', 'Float', 'Boolean', 'Vector', 'Map', 'List', 'Timestamptz', 'Interval', 'TimeZone', 'MemoryVfs', 'Null',
	];
</script>

<div class="flex flex-col gap-2 p-2 border border-gray-200 rounded-lg bg-white shadow-sm {depth > 0 ? "ml-1" : ""}">
	<div class="flex items-center gap-3 border-b border-gray-50">
		{#if label}
			<span class="text-xs font-black text-gray-400 uppercase tracking-tighter min-w-16">{label}</span>
		{/if}
		<div class="w-36">
			<Select id="{id}-typeselect" label="Select type" value={Object.keys(value)[0]} onchange={(currentType) => {
                if(currentType === Object.keys(value)[0]) return
                console.log(currentType, value)
                switch (currentType) {
                    case 'Text': value = { Text: '' }; break;
                    case 'Integer': value = { Integer: 0 }; break;
                    case 'Int64': value = { Int64: '0' }; break;
                    case 'Float': value = { Float: 0.0 }; break;
                    case 'Boolean': value = { Boolean: false }; break;
                    case 'Vector': value = { Vector: [0, 0, 0] }; break;
                    case 'Map': value = { Map: [] }; break;
                    case 'List': value = { List: [] }; break;
                    case 'Timestamptz': value = { Timestamptz: new Date().toISOString() }; break;
                    case 'Interval': value = { Interval: [0, 0] }; break;
                    case 'TimeZone': value = { TimeZone: 'UTC' }; break;
                    case 'MemoryVfs': value = { MemoryVfs: {} }; break;
                    case 'Null': value = { Null: null }; break;
                }
            }} options={types} placeholder="" />
		</div>
	</div>

	<div class="contents">
		{#if 'Text' in value}
			<TextBox id={id} bind:value={value.Text} placeholder="String..." />
		{:else if 'Integer' in value}
			<Number id={id} bind:value={value.Integer} />
		{:else if 'Int64' in value}
			<TextBox id={id} bind:value={value.Int64} placeholder="BigInt..." />
		{:else if 'Float' in value}
			<Number id={id} bind:value={value.Float} />
		{:else if 'Boolean' in value}
			<Checkbox id="bool-input" bind:checked={value.Boolean} label="Enabled" />
		{:else if 'Vector' in value}
			<div class="grid grid-cols-3 gap-2">
				<Number id="{id}-x" bind:value={value.Vector[0]} label="X" />
				<Number id="{id}-y" bind:value={value.Vector[1]} label="Y" />
				<Number id="{id}-z" bind:value={value.Vector[2]} label="Z" />
			</div>
		{:else if 'Timestamptz' in value}
			<TextBox id={id} type="datetime-local" bind:value={value.Timestamptz} />
		{:else if 'Interval' in value}
			<div class="grid grid-cols-2 gap-2">
				<Number id="{id}-sec" bind:value={value.Interval[0]} label="Seconds" />
				<Number id="{id}-ns" bind:value={value.Interval[1]} label="Nanos" />
			</div>
		{:else if 'TimeZone' in value}
			<TextBox id={id} bind:value={value.TimeZone} placeholder="UTC..." />
		{:else if 'Null' in value}
			<!--intentionally empty-->
		{:else if 'List' in value}
			<div class="flex flex-col gap-3 pl-2 border-l-2 border-blue-100">
				{#each value.List as _, i}
					<div class="relative group">
						<Self id={`${id}-le${i}`} bind:value={value.List[i]} label={`#${i}`} depth={depth + 1} />
						<button onclick={() => {
                            if('List' in value) value.List = value.List.filter((_, idx: number) => idx !== i)
                        }} class="absolute top-2 right-2 p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity">✕</button>
					</div>
				{/each}
				<Button onclick={() => {
                    if('List' in value) value.List = [...value.List, { Null: null }]
                }}>+ Add Item</Button>
			</div>
		{:else if 'Map' in value}
			<div class="flex flex-col gap-4 pl-2 border-l-2 border-purple-100">
				{#each value.Map as _, i}
					<div class="flex flex-col gap-2 p-2 bg-gray-50/50 rounded-lg relative group border border-gray-100">
						<Self id={`${id}-me${i}`} bind:value={value.Map[i][0]} label="Key" depth={depth + 1} />
						<Self id={`${id}-me${i}`} bind:value={value.Map[i][1]} label="Val" depth={depth + 1} />
						<button onclick={() => {
                            if('Map' in value) value.Map = value.Map.filter((_: any, idx: number) => idx !== i)
                        }} class="absolute top-2 right-2 p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity">✕</button>
					</div>
				{/each}
				<Button onclick={() => {
                    if('Map' in value) value.Map = [...value.Map, [{ Text: '' }, { Null: null }]]
                }}>+ Add Entry</Button>
			</div>
		{:else if 'MemoryVfs' in value}
			<div class="flex flex-col gap-2 pl-2 border-l-2 border-orange-100">
				{#each Object.keys(value.MemoryVfs) as key}
					<div class="flex items-start gap-2 group bg-gray-50/30 p-2 rounded-lg">
						<div class="flex-1"><TextBox id={`vfs-k-${key}`} value={key} readonly label="Path"/></div>
						<div class="flex-1"><TextBox id={`vfs-v-${key}`} bind:value={value.MemoryVfs[key]} label="Content" /></div>
						<button onclick={() => {
                            if('MemoryVfs' in value) delete value.MemoryVfs[key]
                        }} class="mt-8 p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity">✕</button>
					</div>
				{/each}
				<Button onclick={() => {
                    if('MemoryVfs' in value) {
                        let key = prompt("Path?")
                        if(key) value.MemoryVfs[key] = ""
                    }
                }}>+ Add File</Button>
			</div>
		{/if}
	</div>
</div>