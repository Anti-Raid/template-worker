<script lang="ts">
	import { auth } from '$lib/auth.svelte';
</script>

<div class="p-4 border rounded-lg bg-gray-50">
	<h2 class="text-lg font-semibold mb-2">Home Page</h2>
	<p class="text-sm text-gray-600">API Token:</p>
	<code class="block mt-2 p-2 bg-gray-200 rounded break-all">
		{auth.token || 'No token set'}
	</code>
</div>

<p class="mt-4">Visit <a href="https://svelte.dev/docs/kit" class="text-blue-500 hover:underline">svelte.dev/docs/kit</a> to read the documentation</p>
