# My language vent (old/outdated)

**For reference on my discord vents: https://discord.com/channels/763812938875535361/1040734156327501905/1267195190100361440**

Why is lua the only sane language for embedding

- V8 has big ffi problems with Rust. If you try spawning too many isolates, you have a 100% chance of aborting your process and this issue can only be resolved by performing unsafe void\* pointer casts (although I hear rustyscript has gotten through many of these (?))
- QuickJS is a bit too slow and not very well documented and maintained
- Rhai is good but it’s a custom language and it’s sandboxing abilities need unsafe code to fully work and heap memory limits require you to manually calculate the heap usage (which is hard on multi-tenant systems like AntiRaid)
- Tera has virtually no safety features and will gladly execute an infinite recursion that brings down the whole infrastructure
- For starlark/skylark, go to the point on rhai but (hopefully) without the unsafe bits

I can understand now why the game modding industry uses lua, it’s basically the only sane language for handling user input

Lua is legit the only sane scripting language on this entire list

In addition, Luau is used in Roblox games so it caters to one of Discord's target markets as well

