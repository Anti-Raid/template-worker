<script lang="ts">
    import { Stream } from '$lib/msyscall';
    import { auth } from '$lib/auth.svelte';
    import type { KhronosValue } from '$lib/msyscall/khronosvalue';
    import { encode, dexpand } from '$lib/msyscall/khronosvalue';
    import type { Id } from '$lib/msyscall/types/common';
    import MultiTextBox from '$lib/MultiTextBox.svelte';

    let { id }: { id: Id } = $props();

    let logs = $state<{type: string, msg: string}[]>([]);
    let connected = $state(false);
    let isStreaming = $state(false);
    let client: Stream | null = null;
    let logContainer: HTMLDivElement | undefined = $state();
    
    let topics = $state<string[]>(['print']);
    let activeSubscriptions = new Set<string>();

    function prettyPrint(val: any): string {
        if (val instanceof Map) {
            let str = "";
            for (const [k, v] of val.entries()) {
                str += `${String(k)}=${prettyPrint(v)} `;
            }
            return str.trim() || "{}";
        } else if (Array.isArray(val)) {
            return `[${val.map(v => prettyPrint(v)).join(', ')}]`;
        } else if (val && typeof val === 'object' && !(val instanceof Date)) {
            let str = "";
            for (const [k, v] of Object.entries(val)) {
                str += `${k}=${prettyPrint(v)} `;
            }
            return str.trim() || "{}";
        } else if (typeof val === 'string') {
            return val;
        } else {
            return String(val);
        }
    }

    $effect(() => {
        if (client) {
            client.destroy();
            client = null;
            connected = false;
        }
        if (!isStreaming) {
            return;
        }
        
        logs = []; // Clear logs when switching or starting
        activeSubscriptions.clear();
        
        client = new Stream(
            auth.instanceUrl,
            auth.token || undefined,
            id,
            (msg: KhronosValue) => {
                if (msg instanceof Map) {
                    const t = msg.get('t');
                    const n = msg.get('n');
                    const text = msg.get('msg');
                    
                    if (t === 'msg' && typeof text !== 'undefined') {
                        const topic = String(n);
                        if (topics.includes(topic)) {
                            logs.push({ type: topic, msg: prettyPrint(text) });
                            logs = [...logs];
                            if (logContainer) {
                                setTimeout(() => {
                                    if (logContainer) logContainer.scrollTop = logContainer.scrollHeight;
                                }, 0);
                            }
                        }
                    }
                }
            },
            (status: boolean) => {
                connected = status;
                if (status && client) {
                    for (const topic of topics) {
                        client.send(dexpand(encode({ t: "sub", n: topic })));
                        activeSubscriptions.add(topic);
                    }
                }
            }
        );

        return () => {
            if (client) {
                client.destroy();
                client = null;
            }
        };
    });

    // Handle topic changes
    $effect(() => {
        if (connected && client) {
            const currentTopics = new Set(topics);
            
            // Sub to new topics
            for (const topic of currentTopics) {
                if (!activeSubscriptions.has(topic)) {
                    client.send(dexpand(encode({ t: "sub", n: topic })));
                    activeSubscriptions.add(topic);
                }
            }
            
            // Unsub from removed topics
            for (const topic of activeSubscriptions) {
                if (!currentTopics.has(topic)) {
                    client.send(dexpand(encode({ t: "unsub", n: topic })));
                    activeSubscriptions.delete(topic);
                }
            }
        }
    });
</script>

<div class="border rounded-lg bg-gray-900 overflow-hidden flex flex-col h-125 mt-4 mb-4">
    <div class="bg-gray-800 text-gray-300 px-4 py-3 text-xs font-semibold flex flex-col gap-3 tracking-wider">
        <div class="flex justify-between items-center">
            <div class="flex items-center gap-3">
                <span class="uppercase">Realtime Logs</span>
                <button 
                    class="px-2 py-1 rounded text-[10px] font-bold uppercase transition-colors {isStreaming ? 'bg-red-500 hover:bg-red-600 text-white' : 'bg-green-500 hover:bg-green-600 text-white'}"
                    onclick={() => isStreaming = !isStreaming}
                >
                    {isStreaming ? 'Disconnect' : 'Connect'}
                </button>
            </div>
            <span class={connected ? 'text-green-400 uppercase' : (isStreaming ? 'text-yellow-400 uppercase' : 'text-gray-500 uppercase')}>
                {connected ? 'Connected' : (isStreaming ? 'Connecting...' : 'Disconnected')}
            </span>
        </div>
        <div class="w-full">
            <MultiTextBox id="topics" bind:value={topics} placeholder="Add a topic..." />
        </div>
    </div>
    <div bind:this={logContainer} class="p-3 flex-1 overflow-y-auto text-green-400 font-mono text-sm whitespace-pre-wrap leading-relaxed break-all">
        {#each logs as log}
            <div>
                <span class="text-blue-400">[{log.type}]</span> 
                {log.msg}
            </div>
        {/each}
        {#if logs.length === 0}
            <div class="text-gray-500 italic">No logs yet...</div>
        {/if}
    </div>
</div>
