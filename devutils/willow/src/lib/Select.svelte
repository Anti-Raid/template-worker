<script lang="ts">
  type Option = string | { label: string; value: string };
  
  let { 
    id, 
    value, 
    onchange,
    options = [] as Option[], 
    label = "", 
    disabled = false,
    placeholder = "Select an option..."
  }: { id: string, value: string, onchange: (s: string) => void, options: Option[], label: string, disabled?: boolean, placeholder: string | undefined} = $props();

  let processedOptions = $derived(options.map(opt => {
    if (typeof opt === 'string') {
      return { label: opt, value: opt };
    }
    return opt;
  }));
</script>

<div class="flex flex-col gap-2 mb-4">
  {#if label}
    <label for={id} class="text-sm font-medium text-gray-700">{label}</label>
  {/if}
  <select
    {id}
    value={value}
    onchange={(evt) => {
      onchange((evt.target as any).value)
    }}
    {disabled}
    class="px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-base bg-white disabled:bg-gray-200 disabled:cursor-not-allowed appearance-none"
    style="background-image: url('data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%236b7280%22%20d%3D%22M287%2069.4a17.6%2017.6%200%200%200-13-7.4H18.4c-5%200-9.3%201.8-12.9%205.4A17.6%2017.6%200%200%200%200%2082.2c0%205%201.8%209.3%205.4%2012.9l128%20127.9c3.6%203.6%207.8%205.4%2012.8%205.4s9.2-1.8%2012.8-5.4L287%2095c3.5-3.5%205.4-7.8%205.4-12.8%200-5-1.9-9.2-5.5-12.8z%22/%3E%3C/svg%3E'); background-repeat: no-repeat; background-position: right .7em top 50%; background-size: .65em auto;"
  >
    {#if placeholder}
      <option value="" disabled selected={!value}>{placeholder}</option>
    {/if}
    {#each processedOptions as option}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
</div>
