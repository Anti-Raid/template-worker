# builtins

Builtin core commands on Anti Raid. This replaces the Rust builtins in Anti Raid with more maintainable Luau ones.

## Needed APIs for Core Commands V2

- Statistics API for gathering bot statistics from Luau 
- API to update command usage stats given an event
- Finish up moderation API's in ``@antiraid/discord``
- Finish up object storage Khronos API
- Finish up jobserver Khronos API
- Actually code PrivilegedScriptInitContext and other related structures

## Core/Global Commands Design

1. Only scripts within the ``commands`` repo will have the ability to create global commands through the restricted ``PrivilegedScriptInitContext`` userdata (passed as a third argument). The ``PrivilegedScriptInitContext`` will only have a ``add_global_command`` API to add the global command to the list of global commands to be registered to discord using the separate register globals CLI option (of note: add global command DOES NOT ACTUALLY DIRECTLY REGISTER THE COMMAND TO DISCORD TO AVOID DUPLICATING API REQS). Core commands do NOT have access to any other functionality and MUST be usable even without PrivilegedScriptInitContext for the purpose of dogfooding/examples for other users
2. Initial registration of core commands will occur using a special separate temporary thread which will call init.luau on the commands folder. Most importantly, this means that there is *nothing* special about 'privileged scripts' beyond registration making reasoning over code easier
3. Core Commands will be added to every guild's template cache (so normal slash command dispatch etc. will trigger it)

## Exposed Hooks

- `onBuiltinsLoad()`
- `onStingCreate(sting: Sting)`
- `onStingDelete(sting: Sting, mod: string?, auditReason: string?)` (called prior to deletion)
- `onStingSetExpiration(sting: Sting, reason: string, expiresAt: DateTime)` (called prior to setting sting expiration)
- `onStingDeleteExpiration(sting: Sting)` (called prior to deleting sting expiration)
- `onPunishmentCreate(p: Punishment)`
- `onPunishmentDelete(p: Punishment, mod: string?, auditReason: string?)` (called prior to deletion)
- `onPunishmentSetExpiration(p: Punishment, reason: string, expiresAt: DateTime)` (called prior to setting punishment expiration)
- `onPunishmentDeleteExpiration(p: Punishment)` (called prior to deleting punishment expiration)
- `getAllModLogsActions()` (custom mod log actions which can have log channels etc attached to them)
- `getAllModActions()` (custom moderation actions which can have base stings etc attached to them)