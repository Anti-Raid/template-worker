<script lang="ts">
  let { 
    onclick, 
    children, 
    class: className = "", 
    type = "button" as "button" | "submit" | "reset" | null | undefined, 
    disabled = false,
    variant = "primary"
  } = $props();

  let baseClass = "px-4 py-2 rounded-lg font-medium transition-all duration-200 h-10 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed border shadow-sm";
  
  let variantClasses = $derived.by(() => {
    switch (variant.toLowerCase()) {
        case "danger": 
            return "bg-white border-red-200 text-red-600 hover:bg-red-50 hover:border-red-300 disabled:hover:bg-white";
        case "secondary": 
            return "bg-white border-gray-200 text-gray-700 hover:bg-gray-50 hover:border-gray-300 disabled:hover:bg-white";
        case "primary":
        default: 
            return "bg-blue-600 border-transparent text-white hover:bg-blue-700 hover:shadow disabled:hover:bg-blue-600";
    }
  });
</script>

<button 
  {type}
  {onclick}
  {disabled}
  class="{baseClass} {variantClasses} {className}"
>
  {@render children()}
</button>
