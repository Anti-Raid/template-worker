<script lang="ts">
  type Option = string | { label: string; value: string };

  let {
    id,
    value = $bindable([] as string[]),
    options = [] as Option[],
    label = "",
    description = "",
    disabled = false,
    placeholder = "Select options...",
    onchange
  }: {
    id: string;
    value: string[];
    options: Option[];
    label?: string;
    description?: string;
    disabled?: boolean;
    placeholder?: string;
    onchange?: (values: string[]) => void;
  } = $props();

  let container: HTMLDivElement | null = $state(null);
  let isOpen = $state(false);
  let searchTerm = $state("");

  let processedOptions = $derived(options.map(opt => {
    if (typeof opt === 'string') {
      return { label: opt, value: opt };
    }
    return opt;
  }));

  let selectedMap = $derived(new Set(value));

  let filteredOptions = $derived(
    processedOptions.filter(opt =>
      opt.label.toLowerCase().includes(searchTerm.toLowerCase())
    )
  );

  let hasFilteredSelections = $derived(
    filteredOptions.some(o => selectedMap.has(o.value))
  );

  $effect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (isOpen && container && !container.contains(e.target as Node)) {
        isOpen = false;
      }
    };
    document.addEventListener('click', handleOutsideClick);
    return () => {
      document.removeEventListener('click', handleOutsideClick);
    };
  });

  function toggleValue(val: string) {
    if (disabled) return;
    if (selectedMap.has(val)) {
      value = value.filter(v => v !== val);
    } else {
      value = [...value, val];
    }
    onchange?.(value);
  }

  function removeValue(val: string) {
    if (disabled) return;
    value = value.filter(v => v !== val);
    onchange?.(value);
  }

  function clearAll() {
    if (disabled) return;
    const valuesToRemove = new Set(filteredOptions.map(o => o.value));
    value = value.filter(v => !valuesToRemove.has(v));
    onchange?.(value);
  }

  function selectAll() {
    if (disabled) return;
    const valuesToSelect = filteredOptions.map(o => o.value);
    value = Array.from(new Set([...value, ...valuesToSelect]));
    onchange?.(value);
  }
</script>

<div class="flex flex-col gap-2 mb-4 relative w-full" bind:this={container}>
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700">{label}</label>
  {/if}
  {#if description}
    <p class="text-sm font-medium text-gray-500">{description}</p>
  {/if}

  <!-- Trigger Area -->
  <div
    {id}
    role="combobox"
    aria-expanded={isOpen}
    aria-controls={isOpen ? `${id}-listbox` : undefined}
    aria-haspopup="listbox"
    aria-disabled={disabled}
    tabindex={disabled ? -1 : 0}
    onclick={() => !disabled && (isOpen = !isOpen)}
    onkeydown={(e) => {
      if (disabled) return;
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        isOpen = !isOpen;
      } else if (e.key === 'Escape') {
        isOpen = false;
      }
    }}
    class="flex items-center justify-between w-full px-3 py-1.5 border border-gray-300 rounded-md shadow-sm bg-white text-left text-base focus:outline-none focus-within:ring-2 focus-within:ring-blue-500 focus-within:border-blue-500 disabled:bg-gray-200 disabled:cursor-not-allowed min-h-10.5 transition-all cursor-pointer select-none"
    class:border-blue-500={isOpen}
    class:bg-gray-100={disabled}
    class:cursor-not-allowed={disabled}
  >
    <div class="grow flex flex-wrap gap-1.5 items-center min-w-0 pr-2">
      {#if value.length === 0}
        <span class="text-gray-400 text-sm select-none">{placeholder}</span>
      {:else}
        {#each value as val}
          {@const option = processedOptions.find(o => o.value === val)}
          <span class="inline-flex items-center gap-1 bg-blue-50 text-blue-700 border border-blue-200 text-xs font-semibold px-2 py-0.5 rounded">
            {option ? option.label : val}
            {#if !disabled}
              <button
                type="button"
                aria-label="Remove {option ? option.label : val}"
                onclick={(e) => {
                  e.stopPropagation();
                  removeValue(val);
                }}
                class="hover:bg-blue-100 text-blue-500 rounded-full p-0.5 inline-flex items-center justify-center focus:outline-none cursor-pointer"
              >
                <!-- Close Icon -->
                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            {/if}
          </span>
        {/each}
      {/if}
    </div>
    <div class="flex items-center text-gray-400">
      <svg class="w-5 h-5 transition-transform duration-200" class:rotate-180={isOpen} fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
      </svg>
    </div>
  </div>

  <!-- Dropdown Menu -->
  {#if isOpen}
    <div class="absolute left-0 right-0 z-50 mt-1 bg-white border border-gray-300 rounded-md shadow-lg flex flex-col overflow-hidden w-full top-full">
      <!-- Search Box -->
      <div class="relative border-b border-gray-200 flex items-center">
        <svg class="absolute left-3 w-4 h-4 text-gray-400 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <input
          type="text"
          bind:value={searchTerm}
          placeholder="Search..."
          class="w-full pl-9 pr-8 py-2 text-sm focus:outline-none border-none ring-0 bg-transparent text-gray-700 placeholder-gray-400"
        />
        {#if searchTerm}
          <button
            type="button"
            aria-label="Clear search"
            onclick={() => searchTerm = ""}
            class="absolute right-3 text-gray-400 hover:text-gray-600 focus:outline-none cursor-pointer"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        {/if}
      </div>

      <!-- Action Buttons -->
      {#if processedOptions.length > 0}
        <div class="flex justify-between items-center px-3 py-1.5 bg-gray-50 border-b border-gray-150 text-xs text-blue-600 font-medium select-none">
          <button type="button" class="hover:text-blue-800 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-blue-600" onclick={selectAll}>Select All</button>
          <button type="button" class="hover:text-blue-800 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-blue-600" disabled={!hasFilteredSelections} onclick={clearAll}>Clear All</button>
        </div>
      {/if}

      <!-- Options List -->
      <div id={`${id}-listbox`} class="max-h-60 overflow-y-auto divide-y divide-gray-100" role="listbox">
        {#each filteredOptions as option}
          {@const isSelected = selectedMap.has(option.value)}
          <button
            type="button"
            role="option"
            aria-selected={isSelected}
            onclick={() => toggleValue(option.value)}
            class="w-full text-left px-3 py-2 text-sm hover:bg-gray-50 flex items-center justify-between transition-colors focus:outline-none focus:bg-gray-50 cursor-pointer"
          >
            <span class={isSelected ? "font-semibold text-blue-700" : "text-gray-700"}>
              {option.label}
            </span>
            {#if isSelected}
              <!-- Checkmark Icon -->
              <svg class="w-4 h-4 text-blue-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
              </svg>
            {/if}
          </button>
        {:else}
          <div class="px-3 py-3 text-sm text-gray-500 text-center select-none">
            No options found
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
