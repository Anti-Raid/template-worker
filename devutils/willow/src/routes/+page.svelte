<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { mps, stateKey } from '$lib/mainpagestate.svelte';
	import { Button, Toggle, Checkbox, ErrorBox } from '$lib';
    import TextBox from '$lib/TextBox.svelte';
    import KhronosValue from '$lib/KhronosValue.svelte';
    import { encode } from '$lib/msyscall/khronosvalue';
    import { toDispatchResults, type Component, dispatchResultToSetting, type Page } from '$lib/events.parse';
    import SV2 from '$lib/sv2/SV2.svelte';

	let fetchingUserGuilds = $state(false)
	let dispatchingGuildEvent = $state(false)
	let fetchingSettings = $state(false)
	let _statefileInput: HTMLInputElement;

	let filteredGuilds = $derived.by(() => {
		if (!mps.state.fetchedUserGuilds || typeof mps.state.fetchedUserGuilds === 'string') return [];
		let guildsExist = mps.state.fetchedUserGuilds.guilds_exist
		return mps.state.fetchedUserGuilds.guilds
			.map((guild, i) => ({ guild, exists: guildsExist[i] }))
			.filter(item => !mps.state.showOnlyPresent || item.exists);
	});
</script>

<div class="p-4 border rounded-lg bg-gray-50 mb-4">
	<h2 class="text-lg font-semibold mb-2">Home Page</h2>
	<div class="flex flex-col gap-1">
		<p class="text-sm text-gray-600 font-semibold">Login Token:</p>
		<code class="block p-2 bg-gray-200 rounded break-all text-xs font-mono">
			{auth.token || 'No token set'}
		</code>
	</div>

	<div class="mt-4 border-t border-gray-200 pt-4 flex flex-col gap-2">
		<p class="text-sm text-gray-600 font-semibold">Advanced State Management:</p>
		<div class="flex flex-col gap-2">
			<div class="flex gap-2">
				<Button onclick={() => {
					mps.save()
				}}>Save Current State</Button>
				<Button onclick={() => {
					let exported = mps.export()
					const blob = new Blob([exported], { type: 'application/json' });
					const url = URL.createObjectURL(blob);
					const a = document.createElement('a');
					a.href = url;
					a.download = `export__${stateKey}.json.txt`;
					document.body.appendChild(a);
					a.click();
					document.body.removeChild(a);
					URL.revokeObjectURL(url);

				}}>Dump State</Button>
				<input
					type="file"
					accept=".json,.txt"
					class="hidden"
					bind:this={_statefileInput}
					onchange={async (e) => {
						const file = (e.target as HTMLInputElement).files?.[0];
						if (file) {
							const text = await file.text();
							mps.import(text);
						}
					}}
				/>
				<Button onclick={() => _statefileInput.click()}>Import...</Button>
			</div>
		</div>
	</div>
</div>

{#if auth.token}
	<div class="p-4 border rounded-lg mb-4 flex flex-col gap-2">
		<h2 class="text-lg font-semibold mb-2">Fetch Servers</h2>

		<Checkbox 
			id="refresh" 
			label="Refresh Guilds From API" 
			bind:checked={mps.state.refresh} 
		/>
		<Button disabled={fetchingUserGuilds} onclick={async () => {
			fetchingUserGuilds = true
			try {
				let data = await auth.getUserGuilds(mps.state.refresh)
				mps.state.fetchedUserGuilds = data.data
			} catch (err) {
				mps.state.fetchedUserGuilds = err ? err.toString() : "Unknown error"
			} finally {
				fetchingUserGuilds = false
			}
		}} class="mb-4">
			{fetchingUserGuilds ? "Fetching..." : "Fetch Servers"}
		</Button>

		{#if typeof mps.state.fetchedUserGuilds == "string"}
			<ErrorBox error={mps.state.fetchedUserGuilds} />
		{:else if mps.state.fetchedUserGuilds != null}
			<div class="flex flex-col gap-2 mt-2">
				<div class="flex items-center justify-between mb-2">
					<h3 class="text-md font-medium">Your Servers</h3>
					<Toggle 
						id="show-only-present" 
						label="Only show present" 
						bind:checked={mps.state.showOnlyPresent} 
					/>
				</div>

				<div class="max-h-100 overflow-y-auto p-1 pr-2">
					<div class="grid grid-cols-1 gap-2">
						{#each filteredGuilds as { guild, exists }}
							<button
								onclick={() => {
									if(!exists) return
									if(mps.state.selectedGuild !== null && mps.state.selectedGuild.id == guild.id) {
										mps.state.selectedGuild = null
									} else {
										mps.state.selectedGuild = guild
									}
								}}
								disabled={!exists}
								class="flex items-center gap-3 p-3 border rounded-md bg-white shadow-sm transition-all text-left w-full group {exists ? 'hover:border-blue-400 cursor-pointer' : 'cursor-not-allowed opacity-60'} {mps.state.selectedGuild && mps.state.selectedGuild.id === guild.id ? 'ring-2 ring-blue-500 border-blue-500' : 'border-gray-200'}"
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
									<div class="flex gap-2 mt-1">
										<span class="text-[10px] bg-gray-100 px-1 rounded text-gray-600 font-mono">Perms: {guild.permissions}</span>
										{#if guild.owner}
											<span class="text-[10px] bg-yellow-100 px-1 rounded text-yellow-700 font-medium">Owner</span>
										{/if}
									</div>
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

{#if mps.state.selectedGuild !== null}
	<h2 class="text-lg font-semibold">Now viewing...</h2>
	<div class="flex items-center gap-3 p-3 text-left w-full group">
		<div class="flex items-center gap-3 p-3 text-left w-full group">
			{#if mps.state.selectedGuild.icon}
				<img
					src={`https://cdn.discordapp.com/icons/${mps.state.selectedGuild.id}/${mps.state.selectedGuild.icon}.png`}
					alt={mps.state.selectedGuild.name}
					loading="lazy"
					class="w-10 h-10 rounded-full bg-gray-100"
				/>
			{:else}
			<div class="w-10 h-10 rounded-full bg-gray-200 flex items-center justify-center text-gray-500 font-bold text-sm">
					{mps.state.selectedGuild.name.substring(0, 1)}
			</div>
			{/if}
			<div class="flex-1 min-w-0">
				<p class="text-sm font-medium text-gray-900 truncate">{mps.state.selectedGuild.name}</p>
				<p class="text-xs text-gray-500 truncate">{mps.state.selectedGuild.id}</p>
				<div class="flex gap-2 mt-1">
					<span class="text-[10px] bg-gray-100 px-1 rounded text-gray-600 font-mono">Perms: {mps.state.selectedGuild.permissions}</span>
					{#if mps.state.selectedGuild.owner}
						<span class="text-[10px] bg-yellow-100 px-1 rounded text-yellow-700 font-medium">Owner</span>
					{/if}
				</div>
			</div>
		</div>
		<Button onclick={() => { mps.state.selectedGuild = null}}>Deselect</Button>
	</div>

	<div class="p-4 border rounded-lg mb-4 flex flex-col gap-2">
		<h2 class="text-lg font-semibold mb-2">Dispatch Event</h2>
		<TextBox id="evtname" bind:value={mps.state.dispatchEvent.event} label="Event Name" placeholder="Event Name"/>
		<KhronosValue id="evtvalue" bind:value={mps.state.dispatchEvent.data}/>
		<Button disabled={dispatchingGuildEvent} onclick={async () => {
			if(!mps.state.selectedGuild) return
			dispatchingGuildEvent = true
			try {
				let data = await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, mps.state.dispatchEvent.event, mps.state.dispatchEvent.data)
				mps.state.dispatchEvent.fetched = {data}
			} catch (err) {
				mps.state.dispatchEvent.fetched = err ? err.toString() : "Unknown error"
			} finally {
				dispatchingGuildEvent = false
			}
		}}>{dispatchingGuildEvent ? "Dispatching" : "Dispatch"}</Button>
		{#if typeof mps.state.dispatchEvent.fetched == "string"}
			<ErrorBox error={mps.state.dispatchEvent.fetched} />
		{:else if mps.state.dispatchEvent.fetched?.data}
			<KhronosValue id="evtvalue" value={mps.state.dispatchEvent.fetched.data} disabled />
		{/if}
	</div>

	<div class="p-4 border rounded-lg mb-4 flex flex-col gap-2">
		<h2 class="text-lg font-semibold mb-2">Settings Fetch</h2>
		<Button disabled={fetchingSettings} onclick={async () => {
			if(!mps.state.selectedGuild) return
			fetchingSettings = true
			try {
				let data = await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode({
					type: "fetch_page",
				}))
				let ders = toDispatchResults(data)
				let settingsForTmpls: Record<string, Page> = {}
				let newerrs: [string, string][] = []
				for(const der of ders) {
					if(der.type == "err") {
						newerrs.push([der.id, der.value?.toString() || "Unknown error"])
					} else {
						try {
							settingsForTmpls[der.id] = dispatchResultToSetting(der.value)
						} catch (err) {
							newerrs.push([der.id, err?.toString() || "Unknown error when parsing to setting"])
						}
					}
				}
				mps.state.settings = settingsForTmpls
				mps.state.settingsErr = newerrs
			} catch (err) {
				mps.state.settingsErr = [["*", err ? err.toString() : "Unknown error"]]
			} finally {
				fetchingSettings = false
			}
		}}>{fetchingSettings ? "Fetching Settings" : "Fetch All Settings"}</Button>
		{#each mps.state.settingsErr as [tmplId, err]}
			<h3 class="text-md font-semibold mb-2 text-red-500">Template '{tmplId}'</h3>
			<ErrorBox error={err} />
		{/each}
		{#each Object.entries(mps.state.settings ?? {}) as [tmplId, page]}
			<h3 class="text-md font-semibold mb-2">Template '{tmplId}'</h3>
			<SV2 template={tmplId} comps={page.components} />
		{/each}
	</div>
{/if}