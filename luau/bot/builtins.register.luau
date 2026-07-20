--- Builtins internal registration mechanism to AntiRaid
---
--- Note that this is INTERNAL and specific to how AntiRaid gets builtin state (such as command info etc) from the builtins standard script
---
--- Do not expect this to work outside of builtins in specific
local commands = require "./commands/commands"
local commandList = {}
for _, command in commands.commands do
    if not command or not command.command then
        error(`Command {command} is invalid, doesn't have a \`command\` object associated with it`)
    end
    table.insert(commandList, command.command)
end

return {
    commands = commandList
}
