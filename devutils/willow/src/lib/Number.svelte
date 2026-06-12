<script lang="ts">
  let { 
    id, 
    value = $bindable(0), 
    placeholder = "", 
    label = "", 
    readonly = false,
    min = undefined,
    max = undefined,
    step = "any"
  } = $props();
</script>

<div class="flex flex-col gap-2 mb-4">
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700">{label}</label>
  {/if}
  <input 
    id={id}
    type="number"
    {placeholder} 
    {min}
    {max}
    {step}
    disabled={readonly}
    aria-disabled={readonly}
    bind:value={value} 
    class={
      readonly 
        ? "px-3 py-2 border border-gray-300 rounded-md shadow-sm text-base bg-gray-200 hover:cursor-not-allowed w-full" 
        : "px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-base w-full"
      }
  />
</div>
