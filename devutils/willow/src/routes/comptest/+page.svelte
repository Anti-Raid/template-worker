<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { Button, Checkbox, Toggle } from '$lib';

	let checkboxValue = $state(false);
	let toggleValue = $state(true);
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
</div>

<Button onclick={async () => {
}} class="mb-4">
	Test Button
</Button>
