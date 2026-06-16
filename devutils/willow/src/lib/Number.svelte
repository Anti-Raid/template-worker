<script lang="ts">
  let { id, description="", value = $bindable(0), placeholder = "", label = "", type = "number", readonly = false } = $props();
</script>

<div class="flex flex-col gap-2 mb-4">
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700">{label}</label>
  {/if}
  {#if description}
    <p class="text-sm font-medium text-gray-300">{description}</p>
  {/if}
  <input 
    id={id}
    {type} 
    {placeholder} 
    disabled={readonly}
    aria-disabled={readonly}
    bind:value={value} 
    class={
      readonly 
        ? "px-3 py-2 border border-gray-300 rounded-md shadow-sm text-base bg-gray-200 hover:cursor-not-allowed" 
        : "px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-base"
      }
  />
</div>
