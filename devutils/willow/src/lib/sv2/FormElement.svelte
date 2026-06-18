<script lang="ts">
    import Number from '$lib/Number.svelte';
    import Select from '$lib/Select.svelte';
    import MultiSelect from '$lib/MultiSelect.svelte';
    import MultiTextBox from '$lib/MultiTextBox.svelte';
    import TextBox from '$lib/TextBox.svelte';
    import Toggle from '$lib/Toggle.svelte';
    import type { FormData, FormElement } from '../events.parse';
    import DisplayElement from './DisplayElement.svelte';

	let { el, data = $bindable() }: { el: FormElement, data: Record<string, any> } = $props();
</script>

{#if el.type == "DisplayElement"}
    <DisplayElement el={el.element} />
{:else if el.type == "Text"}
    <TextBox id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter some text here!"} bind:value={data[el.id]} readonly={el.disabled} />
{:else if el.type == "Number"}
    <Number id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter a number here!"} bind:value={data[el.id]} readonly={el.disabled} />
{:else if el.type == "Select.Text"}
    <Select id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} value={data[el.id]} onchange={(v) => data[el.id] = v} options={el.choices} />
{:else if el.type == "Array.Select.Text"}
    <MultiSelect id={el.id} label={el.label} description={el.description} bind:value={data[el.id]} options={el.choices} disabled={el.disabled} />
{:else if el.type == "Array.Text"}
    <MultiTextBox id={el.id} label={el.label} description={el.description} bind:value={data[el.id]} disabled={el.disabled} />
{:else if el.type == "Boolean"}
    <Toggle id={el.id} bind:checked={data[el.id]} label={el.label} disabled={el.disabled}/>
    {#if el.description}
        <p class="text-sm font-medium text-gray-300">{el.description}</p>
    {/if}
{/if}