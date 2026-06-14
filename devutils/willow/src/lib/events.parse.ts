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
    type: "formset_reorder", 
    id: string,
    list: string[]
} | {
    type: "form_action",
    action_button_id: string, 
    formset_id: string,
    form_id: string, 
    form_data?: Record<string, any>
} | {
    type: "fetch_page",
}

const COMPONENT_TYPES = [
    "Section", "FormSet", "DisplayElement"
] as const

const DISPLAY_ELEMENT_TYPES = [
    "Header", "Paragraph"
] as const

export type DisplayElement = {
    /** a header */
    type: "Header",
    text: string
} | {
    /** a paragraph */
    type: "Paragraph",
    text: string
}

/** A base component */
export type Component = {
    /** A raw text block */
    type: "DisplayElement",
    element: DisplayElement
} | {
    /** A section block */
    type: "Section",
    id: string,
    title: string,
    description: string,
    entries: Component[]
} | {
    /* A set of forms (with injected values) expanded from a FormSet in luau */
    type: "#Willow.MultiForm",
    /** formset id */
    id: string, 
    /** if set, a reorder event will be sent with the new list of ids */
    reorderable: boolean,
    /** Form elements */
    forms: Form[],
}

export type Form = {
    /* form id */
    form_id: string,
    /** form title */
    title: string,
    /** form elements */
    form: FormElement[],
}

export type FormElement = {
    type: "DisplayElement",
    element: DisplayElement
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
    value: string
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

const RAW_FORM_ELEMENT_TYPES = [
    "DisplayElement", "Text", "Text.User", "Array.Text", "Number", "Select.Text",
    "Toggle.Checkbox", "Toggle.Slider", "Button.Action"
] as const

// raw form element from luau
type RawFormElement = {
    type: "DisplayElement",
    element: DisplayElement
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
    data: Map<string, RawKhronosValue>,
}

export type Page = {
    components: Component[],
    /** form datas to expand every FormSet into

    for every FormSet, the frontend will expand the given data into the FormSet internally */
    formdata: Map<string, FormData[]>,
}

const _isOneOf = <T extends string>(value: string, allowedList: readonly T[]): value is T =>{
  return (allowedList as readonly string[]).includes(value);
}

const assertOneOf = <T extends string>(value: string, allowedList: readonly T[]): T => {
  if (_isOneOf(value, allowedList)) {
    return value;
  }
  throw new Error(`Invalid value: "${value}". Expected one of: ${allowedList.join(', ')}`);
}

const getTypeName = (value: RawKhronosValue): string => {
    if (typeof value === "string") return value
    return Object.keys(value)[0] || "Unknown"
}

const assertOptional = <T>(value: RawKhronosValue | undefined, fn: (value: RawKhronosValue, ty?: string) => T, ty: string = "optional"): T | undefined => {
    if (value === undefined || value === "Null") return undefined
    if(ty) return fn(value, ty)
    else return fn(value)
}

const assertString = (value: RawKhronosValue, ty: string = "string"): string => {
    if (value === "Null" || !("Text" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.Text
}

const assertNumber = (value: RawKhronosValue, ty: string = "number"): number => {
    if (value === "Null") throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    if("Float" in value) return value.Float
    else if("Integer" in value) return value.Integer
    throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
}

const assertBoolean = (value: RawKhronosValue, ty: string = "boolean"): boolean => {
    if (value === "Null" || !("Boolean" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.Boolean
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

const mapGetOpt = (map: Map<string, RawKhronosValue>, prop: string): RawKhronosValue | undefined => {
    let p = map.get(prop)
    if(!p || p === "Null") return undefined
    return p
}

/**
 * Given a value from a event, returns the DispatchResult created by Luau
 */
export const toDispatchResults = (value: RawKhronosValue): DispatchResult[] => {
    const l = assertList(value, "list of DispatchResult's")
    const results: DispatchResult[] = []
    for(const result of l) {
        const resultMap = assertMap(result)
        const id = assertString(mapGet(resultMap, "id"), "id")
        const type = assertOneOf(assertString(mapGet(resultMap, "type"), "type"), ["ok", "err"])
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
export const dispatchResultToSetting = (value: RawKhronosValue): Component[] => {
    const MAX_DEPTH: number = 10;
    
    const page = assertMap(value, "Page")

    // Extract out formdata first
    const rawFormDataMap = assertMap(mapGet(page, "formdata"), "Page#formdata")
    const formDataMap: Map<string, FormData[]> = new Map()
    for(const [key, formVals] of rawFormDataMap) {
        formDataMap.set(key, assertList(formVals).map((fv, idx) => {
           const map = assertMap(fv, `FormData map at idx ${idx} of Page#formdata`)
           const id = assertString(mapGet(map, "id"), "id")
           const title = assertString(mapGet(map, "title"), "title")
           const data = assertMap(mapGet(map, "data"), "data")
           return {id, title, data}
        }))
    }

    const expandDisplayElement = (map: Map<string, RawKhronosValue>): DisplayElement => {
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), DISPLAY_ELEMENT_TYPES)
        switch (type) {
            case "Header":
            case "Paragraph":
                const text = assertString(mapGet(map, "text"), "text")
                return { type, text }
        }
    }

    const expandRawFormElement = (map: Map<string, RawKhronosValue>): RawFormElement => {
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), RAW_FORM_ELEMENT_TYPES)
        switch (type) {
            case "DisplayElement":
                const delem = expandDisplayElement(assertMap(mapGet(map, "element"), "element"))
                return { type, element: delem }
            case "Text":
            case "Text.User":
            case "Number": // all of these share the same base type (for now)
               const tid = assertString(mapGet(map, "id"), "id")
               const tlabel = assertString(mapGet(map, "label"), "label")
               const tdesc = assertOptional(mapGetOpt(map, "description"), assertString)
               const tph = assertOptional(mapGetOpt(map, "placeholder"), assertString)
               const tdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
               return { type, id: tid, label: tlabel, description: tdesc, placeholder: tph, disabled: tdisabled }
            case "Array.Text":
                const astyle = assertOneOf(assertString(mapGet(map, "style"), "style"), ["Normal", "Kittycat"])
                const aid = assertString(mapGet(map, "id"), "id")
                const alabel = assertString(mapGet(map, "label"), "label")
                const adesc = assertOptional(mapGetOpt(map, "description"), assertString)
                const adisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
                return { type, id: aid, style: astyle, label: alabel, description: adesc, disabled: adisabled }
            case "Select.Text":
                const sid = assertString(mapGet(map, "id"), "id")
                const slabel = assertString(mapGet(map, "label"), "label")
                const sdesc = assertOptional(mapGetOpt(map, "description"), assertString)
                const sdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
                const schoices = assertList(mapGet(map, "choices"), "choices").map((entry, idx) => {
                    const cmap = assertMap(entry, `choices map at idx ${idx} for Select.Text with id ${idx} [${sid}]`) // expand the entry into choices
                    const clabel = assertString(mapGet(cmap, "label"), "label")
                    const cvalue = assertString(mapGet(cmap, "value"), "value")
                    return { label: clabel, value: cvalue }
                })
                return { type, id: sid, label: slabel, description: sdesc, disabled: sdisabled, choices: schoices }
            case "Toggle.Checkbox":
            case "Toggle.Slider":
               const bid = assertString(mapGet(map, "id"), "id")
               const blabel = assertString(mapGet(map, "label"), "label")
               const bdesc = assertOptional(mapGetOpt(map, "description"), assertString)
               const bdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
               return { type, id: bid, label: blabel, description: bdesc, disabled: bdisabled }
            case "Button.Action":
                const abid = assertString(mapGet(map, "id"), "id")
                const abtext = assertString(mapGet(map, "text"), "text")
                const abstyle = assertOneOf(assertString(mapGet(map, "style"), "style"), ["Primary", "Secondary", "Danger"])
                const absendform = assertBoolean(mapGet(map, "send_form"), "send_form")
                return { type, id: abid, text: abtext, style: abstyle, send_form: absendform }
        }
    }

    // may or may not clone underlying data
    const injectFormDataIntoRawFormElement = (rawel: RawFormElement, formdata: FormData): FormElement => {
        switch (rawel.type) {
            case "DisplayElement":
            case "Button.Action":
                return rawel  // action buttons and display elements dont need formdata values injected
            case "Text":
            case "Text.User":
            case "Select.Text":
                return { ...rawel, value: assertString(mapGet(formdata.data, rawel.id)) }
            case "Number":
                return { ...rawel, value: assertNumber(mapGet(formdata.data, rawel.id)) }
            case "Toggle.Checkbox":
            case "Toggle.Slider":
                return { ...rawel, value: assertBoolean(mapGet(formdata.data, rawel.id)) }
            case "Array.Text":
                return { ...rawel, value: assertList(mapGet(formdata.data, rawel.id)).map(v => assertString(v, "string in string array")) }
        }
    }

    const expandComponent = (map: Map<string, RawKhronosValue>, depth: number): Component[] => {
        if(depth > MAX_DEPTH) throw new Error(`Spec violation: above max depth of ${MAX_DEPTH} in expandComponent`)
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), COMPONENT_TYPES)
        switch (type) {
            case "DisplayElement":
                const delem = expandDisplayElement(assertMap(mapGet(map, "element"), "element"))
                return [{ type, element: delem }]
            case "Section":
                const sid = assertString(mapGet(map, "id"), "id")
                const stitle = assertString(mapGet(map, "title"), "title")
                const sdesc = assertString(mapGet(map, "description"), "description")
                const sentries = assertList(mapGet(map, "entries"), "entries").flatMap((entry, idx) => {
                    return expandComponent(assertMap(entry, `component map at idx ${idx} for section with id ${sid}`), depth+1) // recursively expand the entry into more components
                })
                return [{ type: "Section", id: sid, title: stitle, description: sdesc, entries: sentries }]
            case "FormSet":
                const fsid = assertString(mapGet(map, "id"), "id")
                let formdatas = formDataMap.get(fsid)
                if(!formdatas) return []
                const fsreorderable = assertBoolean(mapGet(map, "reorderable"), "reorderable")
                const baseForm = assertList(mapGet(map, "form"), "form").map((formListElem, idx) => {
                    return expandRawFormElement(assertMap(formListElem, `FormElement at idx ${idx} of FormSet \`${fsid}\``))
                })
                const forms: Form[] = []
                for(const form of formdatas) {
                    forms.push({form_id: form.id, title: form.title, form: baseForm.map(x => injectFormDataIntoRawFormElement(x, form)) })
                }
                return [{ type: "#Willow.MultiForm", id: fsid, forms, reorderable: fsreorderable }]
        }
    }

    // extract components out
    const comps = assertList(mapGet(page, "components"), "Page#components").flatMap((x, idx) => {
        return expandComponent(assertMap(x, `Component at idx ${idx} of Page#components`), 0)
    })

    return comps
}