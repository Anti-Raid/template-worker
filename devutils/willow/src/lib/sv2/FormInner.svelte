<script lang="ts">
	import { type Event, type FormElement, type FormAction } from '../events.parse';
	import Button from '$lib/Button.svelte';
    import { auth } from '$lib/auth.svelte';
    import { mps } from '$lib/mainpagestate.svelte';
    import { encode } from '$lib/msyscall/khronosvalue';
    import ErrorBox from '$lib/ErrorBox.svelte';
    import DisplayElement from './DisplayElement.svelte';
    import TextBox from '$lib/TextBox.svelte';
    import MemberSelectSingle from './MemberSelectSingle.svelte';
    import Number from '$lib/Number.svelte';
    import Select from '$lib/Select.svelte';
    import MultiSelect from '$lib/MultiSelect.svelte';
    import MultiTextBox from '$lib/MultiTextBox.svelte';
    import Toggle from '$lib/Toggle.svelte';
    import { Anima, ASP } from '$lib/sv2.anima';

	let { template, form, formid, formidx, actions, formsetid }: {
        template: string,
		formid: string,
        formidx: number,
        form: FormElement[],
        actions: FormAction[],
        formsetid: string
	} = $props();

    let clickedBtns = $state<Record<number, string | null>>({})
    let data = $derived(mps.state.settings[template].formdata[formsetid][formidx].data);
	
    const branchEngine = new Anima({ 
        disableLambda: true, 
        disableDefine: true, 
        maxSteps: 5000 
    });

    const astCache = new Map<string, any>();
    const getAST = (cond: string) => {
        if (!astCache.has(cond)) {
            astCache.set(cond, new ASP(cond).parse());
        }
        return astCache.get(cond);
    };
    const visibleElements = $derived.by(() => {
        const flattenVisible = (elems: FormElement[]): FormElement[] => {
            const result: FormElement[] = [];
            
            for (const el of elems) {
                if (el.type === "Branch") {
                    try {
                        const ast = getAST(el.cond);
                        const isVisible = branchEngine.isTruthy(branchEngine.evaluate(ast, data));
                        
                        if (isVisible) {
                            result.push(...flattenVisible(el.elems));
                        }
                    } catch (error) {
                        console.error(`Branch evaluation failed for cond: ${el.cond}`, error);
                    }
                } else {
                    result.push(el);
                }
            }
            return result;
        };

        return flattenVisible(form);
    });

    const gatherData = (rec: Record<string, any>, elems: FormElement[]) => {
        const sourceData = mps.state.settings[template].formdata[formsetid][formidx].data;
        for(const elem of elems) {
            if (elem.type == "DisplayElement") continue
            if (elem.type === "Branch") {
                const ast = getAST(elem.cond)
                const isVisible = branchEngine.isTruthy(branchEngine.evaluate(ast, sourceData));
                if (isVisible) {
                    gatherData(rec, elem.elems);
                }
                continue;
            }
            rec[elem.id] = sourceData[elem.id]
        }
    }

    const submit = async (abid: string, sendform: boolean) => {
        // Gather data into formdata
        const formdata = Object.create(null)
        if (sendform) {
            gatherData(formdata, form)
        }

		const sve: Event = {
            type: "form_action",
            __tloop_template_id: template,
            form_id: formid,
            formset_id: formsetid,
            action_button_id: abid,
            form_data: sendform ? formdata : undefined
        }

        if (!mps.state.selectedGuild) throw new Error("Guild not selected")
        await auth.dispatchEvent({type: "Guild", id: mps.state.selectedGuild.id}, "WebSettings", encode(sve))
	}
</script>

{#each visibleElements as el}
    {#if el.type == "DisplayElement"}
        <DisplayElement el={el.element} />
    {:else if el.type == "Text"}
        {#if el.choices?.type === "Fixed"}
            <Select id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} value={data[el.id]} onchange={(v) => data[el.id] = v} options={el.choices.choices} />
        {:else if el.choices?.type === "Role"}
            <Select id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} value={data[el.id]} onchange={(v) => data[el.id] = v} options={mps.roleChoices} />
        {:else if el.choices?.type === "Channel"}
            <Select id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} value={data[el.id]} onchange={(v) => data[el.id] = v} options={mps.channelChoices} />
        {:else if el.choices?.type === "Member"}
            <MemberSelectSingle id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={data[el.id]} disabled={el.disabled} />
        {:else}
            <TextBox id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter some text here!"} bind:value={data[el.id]} readonly={el.disabled} />
        {/if}
    {:else if el.type == "Number"}
        <Number id={el.id} label={el.label} description={el.description} placeholder={el.placeholder || "Enter a number here!"} bind:value={data[el.id]} readonly={el.disabled} />
    {:else if el.type == "Array.Text"}
        {#if el.choices?.type === "Fixed"}
            <MultiSelect id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={data[el.id]} options={el.choices.choices} disabled={el.disabled} />
        {:else if el.choices?.type === "Role"}
            <MultiSelect id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={data[el.id]} options={mps.roleChoices} disabled={el.disabled} />
        {:else if el.choices?.type === "Channel"}
            <MultiSelect id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={data[el.id]} options={mps.channelChoices} disabled={el.disabled} />
        {:else}
            <MultiTextBox id={el.id} label={el.label} description={el.description} placeholder={el.placeholder} bind:value={data[el.id]} disabled={el.disabled} />
        {/if}
    {:else if el.type == "Boolean"}
        <Toggle id={el.id} bind:checked={data[el.id]} label={el.label} disabled={el.disabled}/>
        {#if el.description}
            <p class="text-sm font-medium text-gray-300">{el.description}</p>
        {/if}
    {/if}
{/each}

{#each actions as a, i}
    <Button disabled={clickedBtns[i] === null} onclick={async () => {
        clickedBtns[i] = null
        try {
            await submit(a.id, a.send_form)
            delete clickedBtns[i]
        } catch (err) {
            clickedBtns[i] = err?.toString() || "Unknown error sending action"
        }
    }}>
        {a.text} ({a.style})
    </Button>
    {#if typeof clickedBtns[i] === "string"}
        <ErrorBox error={clickedBtns[i]}/>
    {/if}
{/each}