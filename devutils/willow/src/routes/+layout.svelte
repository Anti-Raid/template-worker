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

		loginState = `GotCode(${authData})`

		// Get API session
		let sess = await auth.createLoginSession(authData, `${window.location.origin}/authorize`)
		if(!sess.user) throw new Error("CreateLoginSession did not return a user as it is required to!")
		auth.session = {session: sess.session, token: sess.token, user: sess.user}
		auth.save()
	}
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
			<Button onclick={async () => {
				auth.session = null
				auth.save()
			}} class="mb-4">
				Logout
			</Button>
		{/if}
	</div>

	{@render children()}
</main>
