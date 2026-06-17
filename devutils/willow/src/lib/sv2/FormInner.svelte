<script lang="ts">
	import { toDispatchResults, type Event, type Form, type FormAction } from '../events.parse';
	import FormElementComp from './FormElement.svelte';
	import Button from '$lib/Button.svelte';
    import { auth } from '$lib/auth.svelte';
    import { mps } from '$lib/mainpagestate.svelte';
    import { encode } from '$lib/msyscall/khronosvalue';
    import ErrorBox from '$lib/ErrorBox.svelte';

	let { template, form, actions, formsetid }: {
        template: string,
		form: Form,
        actions: FormAction[]
        formsetid: string
	} = $props();

    let clickedBtns = $state<Record<number, string | null>>({})

	const submit = async (abid: string, sendform: boolean, form: Form) => {
		const sve: Event = {
            type: "form_action",
            __tloop_template_id: template,
            form_id: form.form_id,
            formset_id: formsetid,
            action_button_id: abid,
            form_data: sendform ? Object.fromEntries(
                form.form.filter(x => x.type != "DisplayElement").map(x => [x.id, x.value])
            ) : undefined
        }

        if (!mps.state.selectedGuild) throw new Error("Guild not selected")
        await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode(sve))
	}
</script>

{#each form.form as f, i}
    <FormElementComp bind:el={form.form[i]} />
{/each}

{#each actions as a, i}
    <Button disabled={clickedBtns[i] === null} onclick={async () => {
        clickedBtns[i] = null
        try {
            await submit(a.id, a.send_form, form)
            delete clickedBtns[i]
        } catch (err) {
            clickedBtns[i] = err?.toString() || "Unknown error sending action"
            alert(err?.toString() || "Unknown error sending action")
        }
    }}>
        {a.text} ({a.style})
    </Button>
    {#if typeof clickedBtns[i] === "string"}
        <ErrorBox error={clickedBtns[i]}/>
    {/if}
{/each}