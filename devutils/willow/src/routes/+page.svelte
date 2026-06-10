<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { Button, Toggle, Checkbox, ErrorBox } from '$lib';
	import type { DashboardGuildData } from '$lib/msyscall/types/discord';

	let refresh = $state(false);
	let showOnlyPresent = $state(false);
	let selectedGuildId = $state<string | null>(null);
	let fetchedUserGuilds: DashboardGuildData | string | null = $state(null)
	let fetchingUserGuilds = $state(false)

	let filteredGuilds = $derived.by(() => {
		if (!fetchedUserGuilds || typeof fetchedUserGuilds === 'string') return [];
		let guildsExist = fetchedUserGuilds.guilds_exist
		return fetchedUserGuilds.guilds
			.map((guild, i) => ({ guild, exists: guildsExist[i] }))
			.filter(item => !showOnlyPresent || item.exists);
	});
</script>

<div class="p-4 border rounded-lg bg-gray-50 mb-4">
	<h2 class="text-lg font-semibold mb-2">Home Page</h2>
	<div class="flex flex-col gap-1">
		<p class="text-sm text-gray-600">Login Token:</p>
		<code class="block p-2 bg-gray-200 rounded break-all text-xs">
			{auth.token || 'No token set'}
		</code>
	</div>
	{#if selectedGuildId}
		<div class="mt-4 flex flex-col gap-1">
			<p class="text-sm text-gray-600">Selected Guild:</p>
			<code class="block p-2 bg-blue-100 text-blue-800 rounded font-mono text-xs">
				{selectedGuildId}
			</code>
		</div>
	{/if}
</div>

{#if auth.token}
	<div class="p-4 border rounded-lg mb-4 flex flex-col gap-2">
		<h2 class="text-lg font-semibold mb-2">Fetch Servers</h2>

		<Checkbox 
			id="refresh" 
			label="Refresh Guilds From API" 
			bind:checked={refresh} 
		/>
		<Button disabled={fetchingUserGuilds} onclick={async () => {
			fetchingUserGuilds = true
			try {
				let data = await auth.getUserGuilds(refresh)
				fetchedUserGuilds = data.data
			} catch (err) {
				fetchedUserGuilds = err ? err.toString() : "Unknown error"
			} finally {
				fetchingUserGuilds = false
			}
		}} class="mb-4">
			{fetchingUserGuilds ? "Fetching..." : "Fetch Servers"}
		</Button>

		{#if typeof fetchedUserGuilds == "string"}
			<ErrorBox error={fetchedUserGuilds} />
		{:else if fetchedUserGuilds != null}
			<div class="flex flex-col gap-2 mt-2">
				<div class="flex items-center justify-between mb-2">
					<h3 class="text-md font-medium">Your Servers</h3>
					<Toggle 
						id="show-only-present" 
						label="Only show present" 
						bind:checked={showOnlyPresent} 
					/>
				</div>

				<div class="max-h-100 overflow-y-auto p-1 pr-2">
					<div class="grid grid-cols-1 gap-2">
						{#each filteredGuilds as { guild, exists }}
							<button
								onclick={() => {
									if(!exists) return
									if(selectedGuildId == guild.id) {
										selectedGuildId = null
									} else {
										selectedGuildId = guild.id
									}
								}}
								disabled={!exists}
								class="flex items-center gap-3 p-3 border rounded-md bg-white shadow-sm transition-all text-left w-full group {exists ? 'hover:border-blue-400 cursor-pointer' : 'cursor-not-allowed opacity-60'} {selectedGuildId === guild.id ? 'ring-2 ring-blue-500 border-blue-500' : 'border-gray-200'}"
							>
								{#if guild.icon}
									<img
										src={`https://cdn.discordapp.com/icons/${guild.id}/${guild.icon}.png`}
										alt={guild.name}
										loading="lazy"
										class="w-10 h-10 rounded-full bg-gray-100 {exists ? '' : 'grayscale'}"
									/>
								{:else}
								<div class="w-10 h-10 rounded-full bg-gray-200 flex items-center justify-center text-gray-500 font-bold text-sm {exists ? '' : 'grayscale'}">
										{guild.name.substring(0, 1)}
									</div>
								{/if}
								<div class="flex-1 min-w-0">
									<p class="text-sm font-medium text-gray-900 truncate">{guild.name}</p>
									<p class="text-xs text-gray-500 truncate">{guild.id}</p>
								</div>
								<div class="flex items-center gap-2">
									{#if exists}
										<span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800">
											Bot present
										</span>
									{:else}
										<span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-800">
											Bot missing
										</span>
									{/if}
								</div>
							</button>
						{/each}
					</div>
				</div>
			</div>
		{/if}
	</div>
{/if}