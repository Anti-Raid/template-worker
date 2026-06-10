<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import TextBox from '$lib/TextBox.svelte';
	import Button from '$lib/Button.svelte';
	import { auth } from '$lib/auth.svelte';

	let { children } = $props();
	let isLoggingIn = $state(false);
	let loginState = $state("Prepare");
	let loginError = $state("");
	const login = async () => {
		loginState = "GetBotConfig"
		let bcfg = await auth.getBotConfig()
		loginState = "WaitForUserOauth"
		let loginUrl = `https://discord.com/api/oauth2/authorize?client_id=${bcfg.client_id}&scope=identify%20guilds&redirect_uri=${window.location.origin}/authorize&response_type=code`
		const popup = window.open(loginUrl, "Oauth2 Login", "popup")
		if (!popup) {
			throw new Error("Popup blocked! Please allow popups for this site.");
		}
		// Wait for popup to be done with promises
		const authData: string = await new Promise((resolve, reject) => {   
			// Poll for popup closed 
            const checkPopup = setInterval(() => {
                if (popup.closed) {
                    clearInterval(checkPopup);
                    window.removeEventListener("message", messageListener);
                    reject(new Error("Login popup closed"));
                }
            }, 500);

            const messageListener = (event: MessageEvent<any>) => {
                if (event.origin !== window.location.origin) return;

                if (event.data && event.data.op === "GotCode") {
                    clearInterval(checkPopup);
                    window.removeEventListener("message", messageListener);
                    popup.close(); // Close the popup for the user
                    resolve(event.data.code); // Resolve the promise with the auth code
                }
            };

            window.addEventListener("message", messageListener);
        });

		if(authData == null) {
			throw new Error("User cancelled login request?")
		}

		loginState = `GotCode(${authData})`

		// Get API session
		let sess = await auth.createLoginSession(authData, `${window.location.origin}/authorize`)
		if(!sess.user) throw new Error("CreateLoginSession did not return a user as it is required to!")
		auth.session = {session: sess.session, token: sess.token, user: sess.user}
		auth.save()

		// wipe existing error states
		checkAuthStatus = null
		loginError = ""
	}

	let checkAuthStatus: boolean | null = $state(null);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<header class="p-4 border-b">
	<h1 class="text-2xl font-bold"><a href="/">willow</a></h1>
</header>

<main class="p-4 max-w-2xl mx-auto">
	<div class="mb-8 flex flex-col gap-2">
		{#if !auth.session}
			<div class="flex items-end gap-2">
				<div class="flex-1">
					<TextBox 
						id="instance-url"
						label="Instance URL" 
						placeholder="https://your-instance.com" 
						bind:value={auth.instanceUrl}
					/>
				</div>
			</div>
			<hr />
			<Button onclick={async () => {
				isLoggingIn = true
				try {
					await login()
				} catch (err) {
					console.error(err)
					loginError = err ? err.toString() : "Unknown error"
					// TODO: Handle error
				}
				isLoggingIn = false
			}} class="mb-4">
				{isLoggingIn ? "Logging In..." : "Login"}
			</Button>
			{#if isLoggingIn}
				<p>Login State: {loginState}</p>
			{/if}
			{#if loginError}
				<p class="text-red-700">{loginError}</p>
			{/if}
		{:else}
			<div class="flex items-end gap-2">
				<div class="flex-1">
					<TextBox 
						id="instance-url"
						label="Instance URL" 
						placeholder="https://your-instance.com" 
						value={auth.instanceUrl}
						readonly
					/>
				</div>
			</div>
			<Button onclick={() => {
				auth.session = null
				auth.save()
			}} class="mb-4">
				Logout
			</Button>
<div class="flex items-center gap-4 p-4 border rounded-lg bg-gray-50 mb-4">
				{#if auth.session.user.avatar}
					<img 
						src={`https://cdn.discordapp.com/avatars/${auth.session.user.id}/${auth.session.user.avatar}.png`} 
						alt={auth.session.user.username} 
						class="w-12 h-12 rounded-full shadow-sm"
					/>
				{:else}
					<div class="w-12 h-12 rounded-full bg-blue-100 flex items-center justify-center text-blue-600 font-bold text-lg">
						{auth.session.user.username.substring(0, 1).toUpperCase()}
					</div>
				{/if}
				<div class="flex-1 min-w-0">
					<div class="flex items-baseline gap-2">
						<span class="font-bold text-gray-900 truncate">
							{auth.session.user.global_name ?? auth.session.user.username}
						</span>
						{#if auth.session.user.global_name}
							<span class="text-sm text-gray-500 truncate">@{auth.session.user.username}</span>
						{/if}
					</div>
					<p class="text-xs text-gray-400 font-mono truncate">{auth.session.user.id}</p>
				</div>
			</div>
			<Button onclick={async () => {
				let isAuthorized = await auth.checkAuth()
				checkAuthStatus = isAuthorized
			}} class="mb-4">
				Check Auth
			</Button>

			{#if checkAuthStatus}
				<p class="text-green-400">Session valid!</p>
			{:else if checkAuthStatus != null}
				<p class="text-red-400">Session invalid! Logout and login?</p>
			{/if}
		{/if}
	</div>

	{@render children()}
</main>
