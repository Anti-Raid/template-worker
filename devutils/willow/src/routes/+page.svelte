<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { mps } from '$lib/mainpagestate.svelte';
	import { ErrorBox, Toggle } from '$lib';
    import { encode } from '$lib/msyscall/khronosvalue';
    import { toDispatchResults, dispatchResultToSetting, type Page } from '$lib/events.parse';
    import SV2 from '$lib/sv2/SV2.svelte';

    import { doLogin } from '$lib/actions/login';

	let isLoggingIn = $state(false);
	let loginState = $state("Prepare");
	let loginError = $state("");
	const login = async () => {
		await doLogin((state) => loginState = state);
		loginError = "";
	}

	let fetchingUserGuilds = $state(false);
	let fetchingSettings = $state(false);
	let fetchingGuildBaseInfo = $state(false);

	let filteredGuilds = $derived.by(() => {
		if (!mps.state.fetchedUserGuilds || typeof mps.state.fetchedUserGuilds === 'string') return [];
		let guildsExist = mps.state.fetchedUserGuilds.guilds_exist;
		return mps.state.fetchedUserGuilds.guilds
			.map((guild, i) => ({ guild, exists: guildsExist[i] }))
			.filter(item => !mps.state.showOnlyPresent || item.exists);
	});

    const fetchServers = async () => {
        if (!auth.token || fetchingUserGuilds) return;
        fetchingUserGuilds = true;
        try {
            let data = await auth.getUserGuilds(mps.state.refresh);
            mps.state.fetchedUserGuilds = data.data;
        } catch (err) {
            mps.state.fetchedUserGuilds = err ? err.toString() : "Unknown error";
        } finally {
            fetchingUserGuilds = false;
        }
    };

	const fetchGuildData = async () => {
		if (!mps.state.selectedGuild) return;
		
        if (!mps.state.baseGuildDatas[mps.state.selectedGuild.id]) {
            fetchingGuildBaseInfo = true;
            try {
                let data = await auth.getGuildInfo(mps.state.selectedGuild.id);
                mps.state.baseGuildDatas[mps.state.selectedGuild.id] = data;
                mps.state.baseGuildDatasFetchErrors = {};
            } catch (err) {
                mps.state.baseGuildDatasFetchErrors[mps.state.selectedGuild.id] = err ? err.toString() : "Unknown error";
            } finally {
                fetchingGuildBaseInfo = false;
            }
        }

		fetchingSettings = true;
		try {
			let data = await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode({
				type: "fetch_page",
			}));
			let ders = toDispatchResults(data);
			let settingsForTmpls: Record<string, Page> = {};
			let newerrs: [string, string][] = [];
			for(const der of ders) {
				if(der.type == "err") {
					newerrs.push([der.id, der.value?.toString() || "Unknown error"]);
				} else {
					try {
						settingsForTmpls[der.id] = dispatchResultToSetting(der.value);
					} catch (err) {
						newerrs.push([der.id, err?.toString() || "Unknown error when parsing to setting"]);
					}
				}
			}
			mps.state.settings = settingsForTmpls;
			mps.state.settingsErr = newerrs;
		} catch (err) {
			mps.state.settingsErr = [["*", err ? err.toString() : "Unknown error"]];
		} finally {
			fetchingSettings = false;
		}
	};

    $effect(() => {
        if (auth.token && !mps.state.fetchedUserGuilds) {
            fetchServers();
        }
    });

    let currentGuildId = $state<string | null>(null);
    $effect(() => {
        if (!auth.token) {
            currentGuildId = null;
            return;
        }

        if (mps.state.selectedGuild && mps.state.selectedGuild.id !== currentGuildId) {
            currentGuildId = mps.state.selectedGuild.id;
            fetchGuildData();
        }
    });
</script>

    <main class="max-w-[1600px] mx-auto px-4 py-8">
        {#if !auth.session}
            <div class="max-w-md mx-auto mt-12 bg-white p-8 rounded-2xl shadow-xl border border-gray-100 relative overflow-hidden">
                <div class="absolute top-0 left-0 w-full h-2 bg-linear-to-r from-indigo-500 to-purple-600"></div>
                <div class="text-center mb-8">
                    <div class="w-16 h-16 bg-linear-to-br from-indigo-500 to-purple-600 rounded-2xl shadow-lg flex items-center justify-center mx-auto mb-4">
                        <span class="text-white font-bold text-3xl">A</span>
                    </div>
                    <h2 class="text-2xl font-bold text-gray-900">Welcome to AntiRaid</h2>
                    <p class="text-gray-500 mt-2 text-sm">Sign in with Discord to manage your servers</p>
                </div>
                
                <div class="flex flex-col gap-4">
                    <button 
                        onclick={async () => {
                            isLoggingIn = true;
                            auth.save();
                            try { await login(); } 
                            catch (err) { loginError = err ? err.toString() : "Unknown error"; }
                            isLoggingIn = false;
                        }} 
                        disabled={isLoggingIn}
                        class="w-full py-3 px-4 bg-indigo-600 hover:bg-indigo-700 text-white font-medium rounded-xl transition-all shadow-md hover:shadow-lg disabled:opacity-50 disabled:cursor-not-allowed mt-2 flex justify-center items-center gap-2"
                    >
                        {#if isLoggingIn}
                            <div class="animate-spin h-5 w-5 border-2 border-white/20 border-t-white rounded-full"></div>
                            <span>Logging In...</span>
                        {:else}
                            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M19.75 11.625v.75A7.75 7.75 0 1112 4.625a7.712 7.712 0 016 2.898l-1.077 1.077a6.213 6.213 0 00-4.923-2.475 6.25 6.25 0 106.25 6.25v-.75h-6.25v-1.5h7.75z"/></svg>
                            <span>Login with Discord</span>
                        {/if}
                    </button>
                    
                    {#if loginError}
                        <div class="mt-2 p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100 flex items-start gap-2">
                            <svg class="w-5 h-5 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                            <p>{loginError}</p>
                        </div>
                    {/if}
                </div>
            </div>
        {:else}
            <div class="flex flex-col md:flex-row gap-6 mt-6">
                <!-- Sidebar for Servers -->
                <div class="w-full md:w-1/3 lg:w-1/4 flex flex-col gap-4 md:sticky md:top-24 md:self-start">
                    <div class="flex items-center justify-between">
                        <h2 class="text-xl font-bold text-gray-800 tracking-tight">Your Servers</h2>
                    </div>

                    {#if fetchingUserGuilds}
                        <div class="flex items-center justify-center p-8 bg-gray-50 rounded-xl border border-gray-100">
                            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
                        </div>
                    {:else if typeof mps.state.fetchedUserGuilds == "string"}
                        <ErrorBox error={mps.state.fetchedUserGuilds} />
                    {:else if mps.state.fetchedUserGuilds != null}
                        <div class="flex flex-col gap-2 max-h-64 md:max-h-[calc(100vh-340px)] overflow-y-auto pr-2 custom-scrollbar">
                            {#if filteredGuilds.length === 0}
                                <div class="text-center p-6 bg-gray-50 rounded-xl border border-gray-100">
                                    <p class="text-gray-500 text-sm">No servers found based on your filters.</p>
                                </div>
                            {/if}
                            {#each filteredGuilds as { guild, exists }}
                                <button
                                    onclick={() => {
                                        if (exists) mps.state.selectedGuild = guild;
                                    }}
                                    disabled={!exists}
                                    class="flex items-center gap-4 p-3 rounded-xl transition-all text-left w-full group border
                                    {exists ? 'cursor-pointer hover:shadow-md hover:-translate-y-0.5' : 'cursor-not-allowed opacity-60'}
                                    {mps.state.selectedGuild?.id === guild.id 
                                        ? 'bg-blue-50 border-blue-500 shadow-sm' 
                                        : 'bg-white border-gray-200 hover:border-blue-300'}"
                                >
                                    {#if guild.icon}
                                        <img
                                            src={`https://cdn.discordapp.com/icons/${guild.id}/${guild.icon}.png`}
                                            alt={guild.name}
                                            loading="lazy"
                                            class="w-12 h-12 rounded-full shadow-sm {exists ? '' : 'grayscale'}"
                                        />
                                    {:else}
                                        <div class="w-12 h-12 rounded-full bg-linear-to-br from-blue-100 to-blue-200 flex items-center justify-center text-blue-700 font-bold text-lg shadow-sm {exists ? '' : 'grayscale'}">
                                            {guild.name.substring(0, 1)}
                                        </div>
                                    {/if}
                                    <div class="flex-1 min-w-0">
                                        <p class="text-sm font-bold text-gray-900 truncate">{guild.name}</p>
                                    </div>
                                </button>
                            {/each}
                        </div>
                    {/if}

                    <div class="bg-white p-3 rounded-xl shadow-sm border border-gray-200 flex flex-col gap-1">
                        <Toggle 
                            id="show-only-present" 
                            label="Only show present" 
                            bind:checked={mps.state.showOnlyPresent} 
                            onchange={() => mps.save()}
                        />
                        <Toggle 
                            id="refresh-guilds" 
                            label="Force API Refresh" 
                            bind:checked={mps.state.refresh} 
                            onchange={() => mps.save()}
                        />
                        <button 
                            disabled={fetchingUserGuilds} 
                            onclick={fetchServers}
                            class="w-full py-2 px-3 bg-gray-50 hover:bg-gray-100 text-gray-700 font-medium rounded-lg transition-colors border border-gray-200 text-sm flex justify-center items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {#if fetchingUserGuilds}
                                <div class="animate-spin h-4 w-4 border-2 border-gray-700/20 border-t-gray-700 rounded-full"></div>
                                <span>Fetching...</span>
                            {:else}
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path></svg>
                                <span>Fetch Servers</span>
                            {/if}
                        </button>
                    </div>
                </div>

                <!-- Main Content Area -->
                <div class="w-full md:w-2/3 lg:w-3/4 flex flex-col gap-6">
                    {#if mps.state.selectedGuild}
                        <div class="bg-white rounded-2xl shadow-sm border border-gray-200 overflow-hidden">
                            <!-- Header -->
                            <div class="bg-linear-to-r from-blue-500 to-indigo-600 p-6 text-white flex items-center gap-4">
                                {#if mps.state.selectedGuild.icon}
                                    <img
                                        src={`https://cdn.discordapp.com/icons/${mps.state.selectedGuild.id}/${mps.state.selectedGuild.icon}.png`}
                                        alt={mps.state.selectedGuild.name}
                                        loading="lazy"
                                        class="w-16 h-16 rounded-full shadow-lg border-2 border-white/20"
                                    />
                                {:else}
                                    <div class="w-16 h-16 rounded-full bg-white/20 flex items-center justify-center text-white font-bold text-2xl shadow-lg border-2 border-white/20">
                                        {mps.state.selectedGuild.name.substring(0, 1)}
                                    </div>
                                {/if}
                                <div>
                                    <h2 class="text-2xl font-bold drop-shadow-sm">{mps.state.selectedGuild.name}</h2>
                                    <p class="text-blue-100 text-sm">Dashboard Configuration</p>
                                </div>
                            </div>

                            <div class="p-6">
                                {#if fetchingGuildBaseInfo || fetchingSettings}
                                    <div class="flex flex-col items-center justify-center py-12 gap-4">
                                        <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-indigo-600"></div>
                                        <p class="text-gray-500 font-medium animate-pulse">Loading settings...</p>
                                    </div>
                                {:else}
                                    {#if mps.state.baseGuildDatasFetchErrors[mps.state.selectedGuild.id]}
                                        <ErrorBox error={mps.state.baseGuildDatasFetchErrors[mps.state.selectedGuild.id]} />
                                    {/if}
                                    
                                    {#each mps.state.settingsErr as [tmplId, err]}
                                        <ErrorBox error={`[${tmplId}]: ${err}`} />
                                    {/each}
                                    
                                    <div class="flex flex-col gap-8">
                                        {#each Object.entries(mps.state.settings ?? {}) as [tmplId, page]}
                                            <div class="bg-gray-50 rounded-xl p-5 border border-gray-100 transition-all hover:shadow-md">
                                                <h3 class="text-lg font-bold text-gray-800 mb-4 pb-2 border-b border-gray-200 capitalize">{tmplId.replace(/_/g, ' ')}</h3>
                                                <SV2 template={tmplId} comps={page.components} />
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    {:else}
                        <div class="flex flex-col items-center justify-center h-full min-h-100 text-center bg-gray-50 rounded-2xl border border-gray-200 border-dashed">
                            <div class="w-16 h-16 bg-gray-200 rounded-full flex items-center justify-center mb-4">
                                <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 002-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"></path></svg>
                            </div>
                            <h3 class="text-xl font-bold text-gray-700">No Server Selected</h3>
                            <p class="text-gray-500 mt-2 max-w-sm">Select a server from the sidebar to configure its settings and manage the bot.</p>
                        </div>
                    {/if}
                </div>
            </div>
        {/if}
    </main>

<style>
    .custom-scrollbar::-webkit-scrollbar {
        width: 6px;
    }
    .custom-scrollbar::-webkit-scrollbar-track {
        background: transparent;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb {
        background-color: #cbd5e1;
        border-radius: 20px;
    }
    :global(html) {
        scroll-behavior: smooth;
    }
</style>