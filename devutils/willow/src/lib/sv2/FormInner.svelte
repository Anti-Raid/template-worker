<script lang="ts">
	import type { Event, Form } from '../events.parse';
	import FormElementComp from './FormElement.svelte';
	import Button from '$lib/Button.svelte';
    import { auth } from '$lib/auth.svelte';
    import { mps } from '$lib/mainpagestate.svelte';
    import { encode } from '$lib/msyscall/khronosvalue';
    import DisplayElement from './DisplayElement.svelte';
    import ErrorBox from '$lib/ErrorBox.svelte';

	let { form, formsetid }: {
		form: Form,
        formsetid: string
	} = $props();

    let clickedBtns = $state<Record<number, string | null>>({})

	const submit = async (abid: string, sendform: boolean, form: Form) => {
		const sve: Event = {
            type: "form_action",
            form_id: form.form_id,
            formset_id: formsetid,
            action_button_id: abid,
            form_data: sendform ? Object.fromEntries(
                form.form.filter(x => x.type != "DisplayElement" && x.type != "Button.Action").map(x => [x.id, x.value])
            ) : undefined
        }

        if (!mps.state.selectedGuild) throw new Error("Guild not selected")
        await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode(sve))
	}
</script>

{#each form.form as f, i}
    <!--Special cases for DisplayElement and Button.Action types-->
    {#if f.type == "DisplayElement"}
        <DisplayElement el={f.element} />
    {:else if f.type == "Button.Action"}
        <Button disabled={clickedBtns[i] === null} onclick={async () => {
            clickedBtns[i] = null
            try {
                await submit(f.id, f.send_form, form)
                delete clickedBtns[i]
            } catch (err) {
                clickedBtns[i] = err?.toString() || "Unknown error sending action"
                alert(err?.toString() || "Unknown error sending action")
            }
        }}>
            {f.text} ({f.style})
        </Button>
        {#if typeof clickedBtns[i] === "string"}
            <ErrorBox error={clickedBtns[i]}/>
        {/if}
    {:else}
        <FormElementComp bind:el={form.form[i]} />
    {/if}
{/each}
