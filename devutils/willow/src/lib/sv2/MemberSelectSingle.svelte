<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { mps } from '$lib/mainpagestate.svelte';
  import TextBox from '$lib/TextBox.svelte';
  import Select from '$lib/Select.svelte';
  import Button from '$lib/Button.svelte';
    import ErrorBox from '$lib/ErrorBox.svelte';

  let {
    id,
    value = $bindable(""),
    placeholder = "Enter User ID...",
    label = "",
    description = "",
    disabled = false,
  }: {
    id: string;
    value: string;
    placeholder?: string;
    label?: string;
    description?: string;
    disabled?: boolean;
  } = $props();

  let searchQuery = $state("");
  let members = $state<any[]>([]);
  let isSearching = $state(false);
  let searchError = $state("");

  async function performSearch() {
    const query = searchQuery.trim();
    if (!query) {
      members = [];
      searchError = "Please enter a username or nickname to search.";
      return;
    }

    const guildId = mps.state.selectedGuild?.id;
    if (!guildId) {
      searchError = "No server selected. Please select a server first.";
      return;
    }

    isSearching = true;
    searchError = "";
    members = [];

    try {
      const results = await auth.searchGuildMembers(guildId, query);
      members = results;
      if (members.length === 0) {
        searchError = "No matching members found.";
      }
    } catch (err: any) {
      searchError = err?.toString() || "Failed to search members";
    } finally {
      isSearching = false;
    }
  }
</script>

{#if disabled}
  <TextBox
    {id}
    {label}
    {description}
    value={value}
    readonly={true}
    placeholder="No member ID set"
  />
{:else}
  <!-- Search Input for Server Members -->
  <div class="flex flex-col gap-2 select-none mb-6">
    <label for="{id}-search" class="text-sm font-semibold text-gray-700">Search Server Members</label>
    <div class="flex gap-2">
      <div class="relative flex-1">
        <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
          <svg class="h-5 w-5 text-gray-400" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" clip-rule="evenodd" />
          </svg>
        </div>
        <input
          type="text"
          id="{id}-search"
          bind:value={searchQuery}
          placeholder="Search username or nickname..."
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              performSearch();
            }
          }}
          class="block w-full pl-10 pr-3 py-2 border border-gray-300 rounded-lg shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-base bg-white transition-colors"
        />
      </div>
      <Button
        type="button"
        onclick={performSearch}
        disabled={isSearching}
        class="shrink-0 shadow-sm"
      >
        {isSearching ? "Searching..." : "Search"}
      </Button>
    </div>
    {#if searchError}
      <ErrorBox error={searchError} />
    {/if}
  </div>

  <!-- Standard Select for Searched Members -->
  {#if members.length > 0}
    <Select
      id="{id}-select"
      label="Select Matching Member"
      description="Select a searched member to auto-fill their User ID"
      value={value}
      onchange={(val) => {
        if (val) value = val;
      }}
      options={members.map(m => ({
        label: `${m.nick || m.user.global_name || m.user.username} (${m.user.id})`,
        value: m.user.id
      }))}
      placeholder="Choose a member..."
    />
  {/if}

  <!-- Expandable User ID details block -->
  <details class="mb-4 border border-gray-200 rounded-lg bg-gray-50 overflow-hidden group">
    <summary class="px-4 py-3 text-sm font-semibold text-gray-700 hover:text-gray-900 cursor-pointer select-none focus:outline-none transition-colors hover:bg-gray-100 flex items-center justify-between">
      View / Edit User ID Manually
      <svg class="w-4 h-4 text-gray-400 group-open:rotate-180 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
    </summary>
    <div class="p-4 border-t border-gray-200 bg-white">
      <TextBox
        {id}
        {label}
        {description}
        {placeholder}
        bind:value={value}
      />
    </div>
  </details>
{/if}