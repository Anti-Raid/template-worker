import { decode, type KhronosValue, type RawKhronosValue } from "./msyscall/khronosvalue";

export type DispatchResult = {
    id: string,
    type: "ok",
    value: RawKhronosValue
} | {
    id: string,
    type: "err",
    value: KhronosValue
}

export type Event = {
    type: "collapsible_reorder", 
    id: string,
    list: string[]
} | {
    type: "form_action",
    action_button_id: string, 
    form_id: string, 
    form_data?: Record<string, any>
} | {
    type: "fetch_page",
}

/** A collapsible block that opens up to a list of components when clicked */
export interface CollapsibleBlock {
    id: string,
    label: string,
    /** the components that form the collapsible block */
    entries: Component[]
}

/** A base component */
export type Component = {
    /** A raw text block */
    type: "TextBlock",
    style: "Header" | "Paragraph",
    text: string
} | {
    /** A section block */
    type: "Section",
    id: string,
    title: string,
    description: string,
    entries: Component[]
} | {
    /* A single form (with injected values) expanded from a FormSet in luau */
    type: "#Willow.Form",
    /** formset id */
    id: string, 
    /* form id */
    form_id: string,
    /** form title */
    title: string,
    /** form elements */
    form: FormElement[],
    /** if set, a reorder event will be sent with the new list of ids */
    reorderable: boolean
} | {
    /* A collapsible set of blocks that open up to a set of inner components when clicked */
    type: "Collapsible",
    collapsibles: CollapsibleBlock[],
}

export type FormElement = {
    type: "TextBlock",
    style: "Header" | "Paragraph",
    text: string
} | {
    type: "Text",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
    value: string
} | {
    type: "Text.User",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
    value: string
} | {
    type: "Array.Text",
    style: "Normal" | "Kittycat",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    value: string[]
} | {
    type: "Number",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
    value: number
} | {
    type: "Select.Text",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    choices: {label: string, value: string}[],
    value: number
} | {
    type: "Toggle.Checkbox",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    value: boolean
} | {
    type: "Toggle.Slider",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    value: boolean
} | {
    type: "Button.Action",
    id: string,
    text: string,
    style: "Primary" | "Secondary" | "Danger",
    /** if set to true, will send the entire form state  */
    send_form: boolean, 
}

// raw form element from luau
type RawFormElement = {
    type: "TextBlock",
    style: "Header" | "Paragraph",
    text: string
} | {
    type: "Text",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
} | {
    type: "Text.User",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
} | {
    type: "Array.Text",
    style: "Normal" | "Kittycat",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
} | {
    type: "Number",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
} | {
    type: "Select.Text",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    choices: {label: string, value: string}[],
} | {
    type: "Toggle.Checkbox",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
} | {
    type: "Toggle.Slider",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
} | {
    type: "Button.Action",
    id: string,
    text: string,
    style: "Primary" | "Secondary" | "Danger",
    /** if set to true, will send the entire form state  */
    send_form: boolean, 
}

export type FormData = {
    /*form id*/
    id: string,
    /**form title*/
    title: string,
    /*form data*/
    data: Record<string, any>,
}

export type Page = {
    components: Component[],
    /** form datas to expand every FormSet into

    for every FormSet, the frontend will expand the given data into the FormSet internally */
    formdata: Record<string, FormData[]>,
}

const getTypeName = (value: RawKhronosValue): string => {
    if (typeof value === "string") return value
    return Object.keys(value)[0] || "Unknown"
}

const assertString = (value: RawKhronosValue, ty: string = "string"): string => {
    if (value === "Null" || !("Text" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.Text
}

const assertList = (value: RawKhronosValue, ty: string = "list"): RawKhronosValue[] => {
    if (value === "Null" || !("List" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.List
}

const assertMap = (value: RawKhronosValue, ty = "Map with string keys"): Map<string, RawKhronosValue> => {
    if (value === "Null" || !("Map" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    let mp: Map<string, RawKhronosValue> = new Map()
    for(const [key, val] of value.Map) {
        let k = assertString(key, "Map with string keys")
        mp.set(k, val)
    }
    return mp
}

const mapGet = (map: Map<string, RawKhronosValue>, prop: string, inprop: string = "map"): RawKhronosValue => {
    let p = map.get(prop)
    if(!p) throw new Error(`\`${prop}\` not found in ${inprop}`)
    return p
}

/**
 * Given a value from a event, returns the DispatchResult created by Luau
 */
export const toDispatchResults = (value: RawKhronosValue): DispatchResult[] => {
    const l = assertList(value)
    const results: DispatchResult[] = []
    for(const result of l) {
        const resultMap = assertMap(result)
        const id = assertString(mapGet(resultMap, "id"), "id")
        const type = assertString(mapGet(resultMap, "type"), "type")
        const value = mapGet(resultMap, "value")
        if(type === "ok") {
            results.push({ id, type, value })
        } else {
            const decodedVal = decode(value)
            results.push({ id, type: "err", value: decodedVal })
        }
    }
    return results
}

/**
 * Given the event response (DispatchResult.value for type="ok"), parse the setting
 */
export const dispatchResultToSetting = (value: RawKhronosValue) => {
    const expandComponent = (map: Map<string, RawKhronosValue>) => {
        const type = assertString(mapGet(map, "type"), "type") // get type field in component
        switch (type) {
            case "TextBlock":
        }
    }
}