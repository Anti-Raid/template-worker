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
  <div class="flex flex-col gap-2 select-none">
    <label for="{id}-search" class="text-sm font-medium text-gray-700">Search Server Members</label>
    <div class="flex gap-2">
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
        class="flex-1 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-base bg-white"
      />
      <Button
        type="button"
        onclick={performSearch}
        disabled={isSearching}
        class="shrink-0"
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
  <details class="mb-4 border border-gray-200 rounded-md bg-gray-50/50">
    <summary class="px-3 py-2 text-sm font-semibold text-blue-600 hover:text-blue-800 cursor-pointer select-none focus:outline-none rounded-md transition-colors">
      View / Edit User ID Manually
    </summary>
    <div class="p-3 border-t border-gray-200 bg-white rounded-b-md">
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