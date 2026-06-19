<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { Button, Checkbox, Toggle, MultiSelect, MultiTextBox } from '$lib';
	import MemberSelectSingle from '$lib/sv2/MemberSelectSingle.svelte';

	let checkboxValue = $state(false);
	let toggleValue = $state(true);
	let multiSelectValue = $state(["banana", "orange"]);
	const fruitOptions = [
		{ label: "Apple", value: "apple" },
		{ label: "Banana", value: "banana" },
		{ label: "Orange", value: "orange" },
		{ label: "Mango", value: "mango" },
		{ label: "Strawberry", value: "strawberry" },
		{ label: "Pineapple", value: "pineapple" },
		{ label: "Watermelon", value: "watermelon" }
	];
	let multiTextBoxValue = $state(["svelte", "tailwind"]);
</script>

<div class="p-4 border rounded-lg mb-4 flex flex-col gap-2">
	<h2 class="text-lg font-semibold mb-2">Component Showcase</h2>
	
	<Checkbox 
		id="test-checkbox" 
		label="Enable notifications" 
		bind:checked={checkboxValue} 
	/>

	<Toggle 
		id="test-toggle" 
		label="Dark mode" 
		bind:checked={toggleValue} 
	/>

	<MultiSelect 
		id="test-multiselect" 
		label="Select your favorite fruits" 
		description="Choose one or more fruits from the list" 
		bind:value={multiSelectValue} 
		options={fruitOptions} 
	/>
	<div class="text-sm text-gray-700 bg-gray-50 p-2 rounded border border-gray-200 mb-2">
		<strong>Selected Fruits:</strong> {multiSelectValue.length > 0 ? multiSelectValue.join(', ') : 'None'}
	</div>

	<MultiTextBox 
		id="test-multitextbox" 
		label="Enter tags" 
		description="Type a word/tag and press Enter to add it" 
		bind:value={multiTextBoxValue} 
	/>
	<div class="text-sm text-gray-700 bg-gray-50 p-2 rounded border border-gray-200 mb-2">
		<strong>Tags list:</strong> {multiTextBoxValue.length > 0 ? multiTextBoxValue.join(', ') : 'None'}
	</div>
</div>

<Button onclick={async () => {
}} class="mb-4">
	Test Button
</Button>
