--- Settings v2 events
export type Event = {
    --- reordering of a formset
    type: "formset_reorder", 
    __tloop_template_id: string, -- the template id to dispatch to, used by tloop to selectively dispatch
    --- id of the formset
    id: string,
    --- new ordering of the formset block
    list: {string}
} | {
    --- form action
    type: "form_action",
    __tloop_template_id: string, -- the template id to dispatch to, used by tloop to selectively dispatch
    action_button_id: string, -- the clicked action button
    formset_id: string, -- id of the form set containing the form
    form_id: string, -- the id of the form containing the action button
    form_data: {[string]: any}? -- where key is the element id, value is the inputted data
} | {
    --- fetch the settings page
    type: "fetch_page",
}

export type EventResponse = {
    --- action does not need any client side state change, keep whatever user input the user currently has
    type: "noop"
} | {
    --- update form data
    type: "patch_formdata",
    new_formdata: {[string]: {FormData}}
}

export type DisplayElement = {
    --- a header
    type: "Header",
    text: string
} | {
    --- a paragraph
    type: "Paragraph",
    text: string
} | {
    --- an error
    type: "Error",
    error: string
}

--- A base component
export type Component = {
    --- a display element
    type: "DisplayElement",
    element: DisplayElement
} | {
    --- a section block
    type: "Section",
    id: string,
    title: string,
    description: string,
    entries: {Component}
} | {
    --- A form reference
    type: "FormSet",
    id: string,
    form: {FormElement},
    actions: {FormAction},
    reorderable: boolean -- if set, a reorder event will be sent with the new list of ids
}

--- Form actions
export type FormAction = {
    type: "Button.Event",
    id: string,
    text: string,
    style: "Primary" | "Secondary" | "Danger",
    send_form: boolean, -- if set to true, will send the entire form state 
}

export type Choices = {
    type: "Fixed",
    choices: {{label: string, value: string}}
} | {
    type: "Channel" -- all non-category channels
} | {
    type: "Role"
} | {
    type: "Member"
}

--- Form elements
export type FormElement = {
    --- a display element
    type: "DisplayElement",
    element: DisplayElement
} | {
    -- text input
    type: "Text",
    id: string,
    label: string,
    description: string?,
    placeholder: string?,
    disabled: boolean?,
    choices: Choices?
} | {
    -- datetime input
    type: "DateTime",
    id: string,
    label: string,
    description: string?,
    placeholder: string?,
    disabled: boolean?,
} | {
    -- list of strings
    type: "Array.Text",
    id: string,
    label: string,
    description: string?,
    disabled: boolean?,
    placeholder: string?,
    choices: Choices?
} | {
    type: "Number",
    id: string,
    label: string,
    description: string?,
    placeholder: string?,
    disabled: boolean?,
} | {
    type: "Boolean",
    id: string,
    label: string,
    description: string?,
    disabled: boolean?,
} | {
    type: "Branch",
    id: string, -- branch id, passed into the branch cond as a param as `id`
    cond: string, -- anima s-expression (`props` will be the first param followed by `id`)
    elems: {FormElement} -- if Branch is true
}

export type FormData = {
    --- form id
    id: string,
    --- form title
    title: string,
    --- form data
    data: {[string]: any},
}

export type Page = {
    components: {Component},
    --- form datas to expand every FormSet into
    ---
    --- for every FormSet, the frontend will expand the given data into the FormSet internally
    ---
    formdata: {[string]: {FormData}},
}

return {}