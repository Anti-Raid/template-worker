local Primitives = require "@antiraid-core/primitives"
local KeyManager = require"@antiraid-ext/keymanager"
local typesext = require"@antiraid/typesext"
local datetime = require "@antiraid/datetime"
local isolate = require"@antiraid-ext/isolate"

--- A Script object.
export type Script = {
    --- Name of the script
    read name: string,

    --- Script language
    read language: "luau",

    --- Script content
    read content: typesext.MemoryVfs,

    --- The date and time when the script was created
    read created_at: datetime.DateTime,

    --- Whether or not the script is paused
    read paused: boolean,

    --- The precomputed VFS of the script
    vfs: typesext.Vfs,
}

--- A CreateTemplate object.
export type CreateScript = {
    --- Name of the template
    read name: string,

    --- Template language
    read language: "luau",

    --- Template Content
    read content: typesext.MemoryVfs,

    --- Whether or not the template is paused
    paused: boolean,
}

export type ScriptManager = {
    --- Returns a list of all scripts
    list: () -> {[string]: Script},
    --- Returns a count of the number of custom scripts we have
    countcustom: () -> number,
    --- Returns a template by key. Will either return `nil` or error on non-custom scripts
    getcustom: (key: string) -> Script?,
    --- Creates or updates a custom script
    setcustom: (data: CreateScript) -> (),
    --- Deletes a custom template
    deletecustom: (key: string) -> (),
}

--- Internal storage type for scripts (the item.value in KV)
type IScriptStore = {
    content: typesext.MemoryVfs,
    paused: boolean,
    type: string,
    language: "luau",
}

--- A data fetcher for templates.
local function TemplateManager(ctx: Primitives.TemplateContext, expose: {[string]: any}): ScriptManager
    local exposedvfs = ctx.btd().base_vfs
    local VfsTemplatingTypes = exposedvfs.TemplatingTypes or error("Internal error: TemplatingTypes VFS not found in exposed VFSs")
    
    local function _parseCustomTemplate(item: KeyManager.KeyRecord<IScriptStore>): Script
        return {
            name = item.key,
            language = item.value.language,
            content = item.value.content,
            created_at = item.createdat,
            last_updated_at = item.lastupdatedat,
            paused = item.value.paused,
            vfs = typesext.Vfs.newoverlay({
                item.value.content, -- newoverlay automatically handles memoryvfs
                VfsTemplatingTypes,
            })
        }
    end

    local function createExpose(tmplName: string)
        local base = {}
        if expose then
            for k, v in expose do base[k] = v end
        end
        base.print = function(...)
            local str = (typesext.fmtpretty :: any)(...)
            if ctx.stream then
                ctx.stream.publish("print", { type = "Log", message = str, source = tmplName })
            end
        end
        return base
    end

    local self = {}

    -- Initialize
    local templates: {[string]: Script} = {}
    local templatedb = KeyManager<<IScriptStore>>(ctx, "builtins.templates", function(records, km)
        for _, template in records do 
            local tmpl = _parseCustomTemplate(template)
            templates[template.key] = tmpl
            if not tmpl.paused then
                ctx.loop.attach(isolate.new("template/"..tmpl.name, tmpl.vfs, createExpose(tmpl.name)))
                ctx.loop.dispatchSingle({name = "OnStartup", data = { reason = "startup" }}, "template/"..tmpl.name)
            end
        end

        return nil
    end)

    local function list(): {[string]: Script}
        return templates
    end

    local function countcustom(): number
        return templatedb.count()
    end

    local function getcustom(key: string): Script?
        return templates[key]
    end

    local function setcustom(data: CreateScript)
        local storedata: IScriptStore = {
            type = "custom",
            content = data.content,
            paused = data.paused,
            language = data.language,
        }
        if not templatedb.exists(data.name) then 
            templatedb.add(storedata, data.name)
        else
            templatedb.updatedata(data.name, storedata)
        end

        local tmpl = templatedb.get(data.name)
        assert(tmpl, "internal error: template not inserted by add call")
        local parsedtmpl = _parseCustomTemplate(tmpl)
        templates[data.name] = parsedtmpl

        if not parsedtmpl.paused then
            ctx.loop.attach(isolate.new("template/"..parsedtmpl.name, parsedtmpl.vfs, createExpose(parsedtmpl.name)))
            ctx.loop.dispatchSingle({name = "OnStartup", data = { reason = "updateTemplateCache" }}, "template/"..parsedtmpl.name)
        else 
            ctx.loop.detach("template/"..parsedtmpl.name) -- detach paused isolate
        end
    end

    local function deletecustom(key: string): ()
        templatedb.remove(key)
        templates[key] = nil
        ctx.loop.detach("template/"..key)
    end

    self.list = list
    self.countcustom = countcustom
    self.getcustom = getcustom
    self.setcustom = setcustom
    self.deletecustom = deletecustom

    return self
end

return {
    TemplateManager = TemplateManager,
}