local settings = require("@antiraid-core/settings")
local array_metatable = require"@antiraid/interop".array_metatable

-- Optional parameter types for clean method signatures
type TextOpts = { description: string?, placeholder: string?, disabled: boolean? }
type TextUserOpts = { description: string?, placeholder: string?, disabled: boolean? }
type ArrayTextOpts = { description: string?, disabled: boolean?, placeholder: string?, }
type NumberOpts = { description: string?, placeholder: string?, disabled: boolean? }
type SelectOpts = { description: string?, placeholder: string?, disabled: boolean? }
type ToggleOpts = { description: string?, disabled: boolean? }

export type FormBuilder = {
    display: (self: FormBuilder, element: settings.DisplayElement) -> FormBuilder,
    text: (self: FormBuilder, id: string, label: string, opts: TextOpts?, choices: settings.Choices?) -> FormBuilder,
    datetime: (self: FormBuilder, id: string, label: string, opts: TextOpts?) -> FormBuilder,
    array_text: (self: FormBuilder, id: string, label: string, opts: ArrayTextOpts?, choices: settings.Choices?) -> FormBuilder,
    number: (self: FormBuilder, id: string, label: string, opts: NumberOpts?) -> FormBuilder,
    boolean: (self: FormBuilder, id: string, label: string, opts: ToggleOpts?) -> FormBuilder,
    branch: (self: FormBuilder, id: string, cond: string, ifbranch: FormBuilder) -> FormBuilder,

    --- Creates a button action which then dispatches a settings action when pressed
    button: (self: FormBuilder, id: string, text: string, style: "Primary" | "Secondary" | "Danger", send_form: boolean) -> FormBuilder,
    
    --- Return the built form elements
    build: (self: FormBuilder) -> ({settings.FormElement}, {settings.FormAction})
}

local function FormBuilderWith(elements: {settings.FormElement}, actions: {settings.FormAction}): FormBuilder
    local self = {}

    function self:display(element: settings.DisplayElement): FormBuilder
        table.insert(elements, { type = "DisplayElement" :: "DisplayElement", element = element })
        return self
    end

    function self:text(id: string, label: string, opts: TextOpts?, choices: settings.Choices?): FormBuilder
        opts = opts or {} :: TextOpts
        table.insert(elements, { type = "Text" :: "Text", id = id, label = label, description = opts.description, placeholder = opts.placeholder, disabled = opts.disabled, choices = choices })
        return self
    end

    function self:datetime(id: string, label: string, opts: TextOpts?): FormBuilder
        opts = opts or {} :: TextOpts
        table.insert(elements, { type = "DateTime" :: "DateTime", id = id, label = label, description = opts.description, placeholder = opts.placeholder, disabled = opts.disabled })
        return self
    end

    function self:array_text(id: string, label: string, opts: ArrayTextOpts?, choices: settings.Choices?): FormBuilder
        opts = opts or {} :: ArrayTextOpts
        table.insert(elements, { type = "Array.Text" :: "Array.Text", id = id, label = label, description = opts.description, placeholder = opts.placeholder, disabled = opts.disabled, choices = choices })
        return self
    end

    function self:number(id: string, label: string, opts: NumberOpts?): FormBuilder
        opts = opts or {} :: NumberOpts
        table.insert(elements, { type = "Number" :: "Number", id = id, label = label, description = opts.description, placeholder = opts.placeholder, disabled = opts.disabled })
        return self
    end

    function self:boolean(id: string, label: string, opts: ToggleOpts?): FormBuilder
        opts = opts or {} :: ToggleOpts
        table.insert(elements, { type = "Boolean" :: "Boolean", id = id, label = label, description = opts.description, disabled = opts.disabled })
        return self
    end

    function self:button(id: string, text: string, style: "Primary" | "Secondary" | "Danger", send_form: boolean): FormBuilder
        table.insert(actions, { type = "Button.Event" :: "Button.Event", id = id, text = text, style = style, send_form = send_form })
        return self
    end

    function self:branch(id: string, cond: string, ifbranch: FormBuilder): FormBuilder 
        local fe, a = ifbranch:build()
        if #a > 0 then error("Branches cannot contain actions") end
        table.insert(elements, { type = "Branch" :: "Branch", id = id, cond = cond, elems = fe })
        return self
    end

    -- workaround for type solver bugs
    local function _build(self: FormBuilder): ({settings.FormElement}, {settings.FormAction}) return elements, actions end
    self.build = _build

    return self
end

local function FormBuilder(): FormBuilder
    return FormBuilderWith(setmetatable({}, array_metatable), setmetatable({}, array_metatable))
end

--- A scoped builder for inside sections
export type SectionBuilder<T> = {
    data: T,
    display: (self: SectionBuilder<T>, element: settings.DisplayElement) -> SectionBuilder<T>,
    formset: (self: SectionBuilder<T>, id: string, builder: FormBuilder, reorderable: boolean) -> SectionBuilder<T>,
    addformdata: (self: SectionBuilder<T>, id: string, fd: settings.FormData) -> SectionBuilder<T>,
}

local function sectionBuilder<T>(data: T, components: {settings.Component}, formdata: {[string]: {settings.FormData}}): SectionBuilder<T>
    local self = {}
    
    function self:display(element: settings.DisplayElement): SectionBuilder<T>
        table.insert(components, { type = "DisplayElement" :: "DisplayElement", element = element })
        return self
    end

    function self:formset(id: string, builder: FormBuilder, reorderable: boolean): SectionBuilder<T>
        local form, actions = builder:build()
        table.insert(components, { type = "FormSet" :: "FormSet", id = id, form = form, actions = actions, reorderable = reorderable })
        return self
    end

    function self:addformdata(id: string, fd: settings.FormData): SectionBuilder<T> 
        if not formdata[id] then formdata[id] = {fd} 
        else table.insert(formdata[id], fd) end
        return self
    end

    self.data = data
    return self
end

export type PageBuilder<T> = {
    data: T,
    --- Add a section to the page
    section: (self: PageBuilder<T>, id: string, title: string, description: string, cb: (sb: SectionBuilder<T>) -> ()) -> PageBuilder<T>,    
    display: (self: PageBuilder<T>, element: settings.DisplayElement) -> PageBuilder<T>,
    formset: (self: PageBuilder<T>, id: string, builder: FormBuilder, reorderable: boolean) -> PageBuilder<T>,
    addformdata: (self: PageBuilder<T>, id: string, fd: settings.FormData) -> PageBuilder<T>,

    --- Return the built page
    build: (self: PageBuilder<T>) -> settings.Page,
}

local function PageBuilder<T>(data: T): PageBuilder<T> 
    local components: {settings.Component} = setmetatable({}, array_metatable) :: any
    local formdata: {[string]: {settings.FormData}} = {}
    local self = {}

    function self:section(id: string, title: string, description: string, cb: (sb: SectionBuilder<T>) -> ()): PageBuilder<T>
        local seccomps: {settings.Component} = setmetatable({}, array_metatable) :: any

        -- the passed section builder will add values directly to the passed comp/formdata ref
        local sb = sectionBuilder(self.data, seccomps, formdata)
        cb(sb)
        
        -- Extract the scoped components 
        table.insert(components, {
            type = "Section" :: "Section",
            id = id,
            title = title,
            description = description,
            entries = seccomps
        })

        return self
    end

    function self:formset(id: string, builder: FormBuilder, reorderable: boolean): PageBuilder<T>
        local form, actions = builder:build()
        table.insert(components, { type = "FormSet" :: "FormSet", id = id, form = form, actions = actions, reorderable = reorderable })
        return self
    end

    function self:addformdata(id: string, fd: settings.FormData): PageBuilder<T> 
        if not formdata[id] then formdata[id] = {fd} 
        else table.insert(formdata[id], fd) end
        return self
    end

    function self:display(element: settings.DisplayElement): PageBuilder<T>
        table.insert(components, { type = "DisplayElement" :: "DisplayElement", element = element })
        return self
    end

    function self:build()  
        return { components = components, formdata = formdata }
    end

    self.data = data
    return self :: PageBuilder<T>
end

return table.freeze{ FormBuilder = FormBuilder, FormBuilderWith = FormBuilderWith, PageBuilder = PageBuilder }