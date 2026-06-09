<script lang="ts">
  let { onclick, children, class: className = "", type = "button" as "button" | "submit" | "reset" | null | undefined } = $props();
</script>

<button 
  {type}
  {onclick}
  class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors h-10.5 font-medium {className}"
>
  {@render children()}
</button>
