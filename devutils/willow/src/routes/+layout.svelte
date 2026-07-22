<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
    import { auth } from '$lib/auth.svelte';
    import { config } from '$lib/config';
    import { resolve } from '$app/paths';

	let { children } = $props();
    let showUserMenu = $state(false);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet">
</svelte:head>

<div class="min-h-screen bg-gray-50 font-['Inter',sans-serif] text-gray-900 flex flex-col">
    <header class="bg-white/80 backdrop-blur-md border-b border-gray-200 sticky top-0 z-50 transition-all shadow-sm">
        <div class="max-w-[1600px] mx-auto px-4 h-16 flex items-center justify-between w-full">
            <div class="flex items-center gap-2">
                <img src="/logo.webp" alt="AntiRaid" class="w-8 h-8 rounded-lg shadow-sm" />
                <h1 class="text-xl font-bold tracking-tight text-gray-900"><a href={resolve('/')}>AntiRaid</a></h1>
            </div>
            
            <div class="flex items-center gap-4">
                <a href={config.inviteUrl} target="_blank" class="text-sm font-medium bg-blue-100 hover:bg-blue-200 text-blue-700 px-4 py-2 rounded-lg transition-colors border border-blue-200 flex items-center">
                    Invite Bot
                </a>
                {#if auth.session}
                    <div class="relative">
                        <button 
                            class="flex items-center gap-3 bg-gray-50/80 hover:bg-gray-100 px-3 py-1.5 rounded-full border border-gray-200 shadow-sm transition-colors cursor-pointer"
                            onclick={() => showUserMenu = !showUserMenu}
                        >
                            {#if auth.session.user.avatar}
                                <img 
                                    src={`https://cdn.discordapp.com/avatars/${auth.session.user.id}/${auth.session.user.avatar}.png`} 
                                    alt={auth.session.user.username} 
                                    class="w-8 h-8 rounded-full shadow-sm"
                                />
                            {:else}
                                <div class="w-8 h-8 rounded-full bg-indigo-100 flex items-center justify-center text-indigo-600 font-bold text-sm">
                                    {auth.session.user.username.substring(0, 1).toUpperCase()}
                                </div>
                            {/if}
                            <div class="hidden sm:block text-sm font-medium">
                                {auth.session.user.global_name ?? auth.session.user.username}
                            </div>
                            <svg class="w-4 h-4 text-gray-500 {showUserMenu ? 'rotate-180' : ''} transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                        </button>
                        {#if showUserMenu}
                            <div class="absolute right-0 mt-1 w-48 bg-white rounded-xl shadow-lg border border-gray-100 py-1 z-50">
                                <button 
                                    class="w-full text-left px-4 py-2 text-sm text-red-600 hover:bg-red-50 hover:text-red-700 transition-colors flex items-center gap-2"
                                    onclick={() => { auth.session = null; auth.save(); showUserMenu = false; }}>
                                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"></path></svg>
                                    Logout
                                </button>
                            </div>
                        {/if}
                    </div>
                {/if}
            </div>
        </div>
    </header>

    <div class="flex-1 w-full">
        {@render children()}
    </div>
</div>
