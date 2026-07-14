<script lang="ts">
  import { errorString } from './msyscall/index';
  import type { MSyscallError } from './msyscall/syscall';

  let { error, class: className = "" } = $props<{
    error: string | MSyscallError | null | undefined;
    class?: string;
  }>();

  let message = $derived.by(() => {
    if (!error) return null;
    if (typeof error === 'string') return error;
    return errorString(error);
  });
</script>

{#if message}
  <div class="p-4 mb-4 text-sm text-red-800 rounded-lg bg-red-50 border border-red-200 flex items-start gap-3 {className}" role="alert">
    <svg class="shrink-0 inline w-4 h-4 mt-0.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
      <path d="M10 .5a9.5 9.5 0 1 0 9.5 9.5A9.51 9.51 0 0 0 10 .5ZM9.5 4a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3ZM12 15H8a1 1 0 0 1 0-2h1v-3H8a1 1 0 0 1 0-2h2a1 1 0 0 1 1 1v4h1a1 1 0 0 1 0 2Z"/>
    </svg>
    <span class="sr-only">Error</span>
    <code class="whitespace-pre-wrap font-sans text-md">{message}</code>
  </div>
{/if}

