<svelte:head>
<script lang="ts">
    let searchParams = new URLSearchParams(window.location.search)
    if(window.opener) {
        window.opener.postMessage({op: "GotCode", code: searchParams.get("code")})
    }
</script>
</svelte:head>
<p>Please wait...</p>