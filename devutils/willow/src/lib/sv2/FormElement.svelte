<script lang="ts">
    import Number from '$lib/Number.svelte';
    import Select from '$lib/Select.svelte';
    import MultiSelect from '$lib/MultiSelect.svelte';
    import MultiTextBox from '$lib/MultiTextBox.svelte';
    import TextBox from '$lib/TextBox.svelte';
    import Toggle from '$lib/Toggle.svelte';
    import type { FormElement } from '../events.parse';

	let { el = $bindable() }: { el: FormElement } = $props();
</script>

{#if el.type == "Button.Action" || el.type == "DisplayElement"}
    <p>unreachable</p> <!-- Handled by outer MultiForm.svelte -->
{:else if el.type == "Text"}
    <TextBox id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter some text here!"} bind:value={el.value} readonly={el.disabled} />
{:else if el.type == "Number"}
    <Number id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter a number here!"} bind:value={el.value} readonly={el.disabled} />
{:else if el.type == "Select.Text"}
    <Select id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} value={el.value} onchange={(v) => el.value = v} options={el.choices} />
{:else if el.type == "Array.Select.Text"}
    <MultiSelect id={el.id} label={el.label} description={el.description} bind:value={el.value} options={el.choices} disabled={el.disabled} />
{:else if el.type == "Array.Text"}
    <MultiTextBox id={el.id} label={el.label} description={el.description} bind:value={el.value} disabled={el.disabled} />
{:else if el.type == "Boolean"}
    <Toggle id={el.id} bind:checked={el.value} label={el.label} disabled={el.disabled}/>
    {#if el.description}
        <p class="text-sm font-medium text-gray-300">{el.description}</p>
    {/if}
{/if}