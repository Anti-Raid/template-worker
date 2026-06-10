<script lang="ts">
  let { id, checked = $bindable(false), label = "", disabled = false } = $props();
</script>

<div class="flex items-center gap-3 mb-4">
  <button
    type="button"
    id={id}
    role="switch"
    aria-checked={checked}
    aria-label={label}
    {disabled}
    onclick={() => !disabled && (checked = !checked)}
    class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 {checked ? 'bg-blue-600' : 'bg-gray-200'}"
  >
    <span
      aria-hidden="true"
      class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out {checked ? 'translate-x-5' : 'translate-x-0'}"
    ></span>
  </button>
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700 cursor-pointer select-none">
      {label}
    </label>
  {/if}
</div>
