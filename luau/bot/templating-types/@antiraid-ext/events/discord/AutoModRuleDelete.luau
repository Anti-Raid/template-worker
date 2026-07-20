local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- AutoModRuleDelete
local function AutoModRuleDelete(callback: (ctx: Primitives.TemplateContext, rule: discord.AutomoderationRuleObject) -> ())
    return createTab("AUTO_MODERATION_RULE_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return AutoModRuleDelete