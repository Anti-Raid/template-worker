<script lang="ts">
  let { id, checked = $bindable(false), label = "", disabled = false } = $props();
</script>

<div class="flex items-center gap-3 mb-4">
  <input
    type="checkbox"
    id={id}
    bind:checked={checked}
    {disabled}
    class="w-5 h-5 text-blue-600 border-gray-300 rounded focus:ring-blue-500 focus:ring-2 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
  />
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700 cursor-pointer select-none">
      {label}
    </label>
  {/if}
</div>
