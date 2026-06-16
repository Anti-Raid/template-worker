<script lang="ts">
  let {
    id,
    value = $bindable([] as string[]),
    label = "",
    description = "",
    placeholder = "Type and press Enter...",
    disabled = false
  }: {
    id: string;
    value: string[];
    label?: string;
    description?: string;
    placeholder?: string;
    disabled?: boolean;
  } = $props();

  let inputValue = $state("");
  let currentPlaceholder = $derived(value.length === 0 ? placeholder : "Add more...");

  function addTag() {
    if (disabled) return;
    const trimmed = inputValue.trim();
    if (trimmed && !value.includes(trimmed)) {
      value = [...value, trimmed];
      inputValue = "";
    }
  }

  function removeTag(index: number) {
    if (disabled) return;
    value = value.filter((_, i) => i !== index);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      addTag();
    } else if (e.key === 'Backspace' && inputValue === '' && value.length > 0) {
      removeTag(value.length - 1);
    }
  }
</script>

<div class="flex flex-col gap-2 mb-4 relative w-full">
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700">{label}</label>
  {/if}
  {#if description}
    <p class="text-sm font-medium text-gray-500">{description}</p>
  {/if}

  <!-- Input Container -->
  <div
    class="flex flex-wrap gap-1.5 items-center px-3 py-1.5 border border-gray-300 rounded-md shadow-sm bg-white focus-within:ring-2 focus-within:ring-blue-500 focus-within:border-blue-500 min-h-10.5 transition-all cursor-text"
    class:bg-gray-100={disabled}
    class:cursor-not-allowed={disabled}
  >
    {#each value as tag, index}
      <span class="inline-flex items-center gap-1 bg-gray-150 text-gray-800 border border-gray-200 text-xs font-semibold px-2 py-0.5 rounded">
        {tag}
        {#if !disabled}
          <button
            type="button"
            aria-label="Remove {tag}"
            onclick={() => removeTag(index)}
            class="hover:bg-gray-200 text-gray-500 rounded-full p-0.5 inline-flex items-center justify-center focus:outline-none cursor-pointer"
          >
            <!-- Close Icon -->
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        {/if}
      </span>
    {/each}
    <input
      type="text"
      {id}
      bind:value={inputValue}
      onkeydown={handleKeyDown}
      onblur={addTag}
      placeholder={currentPlaceholder}
      {disabled}
      class="grow min-w-30 focus:outline-none border-none ring-0 p-0 text-base bg-transparent text-gray-900 placeholder-gray-400 disabled:cursor-not-allowed"
    />
  </div>
</div>
